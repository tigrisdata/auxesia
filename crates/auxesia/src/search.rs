//! A snapshot-owned index shared by the website and MCP.
use crate::{
    content::Site,
    render::{self, Route},
};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tantivy::{
    Index, IndexReader, TantivyDocument, Term,
    collector::TopDocs,
    query::{BooleanQuery, BoostQuery, Occur, PhraseQuery, Query, TermQuery},
    schema::{Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions},
    snippet::SnippetGenerator,
    tokenizer::{LowerCaser, SimpleTokenizer, TextAnalyzer},
};

pub const HTML_PATH: &str = "/_auxesia/search";
pub const JSON_PATH: &str = "/_auxesia/search.json";

#[derive(Clone, Debug, Deserialize)]
pub struct SearchRequest {
    #[serde(default, alias = "q")]
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    10
}

impl SearchRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.query.chars().count() > 512 {
            return Err("The query must contain at most 512 characters.".into());
        }
        if !(1..=50).contains(&self.limit) {
            return Err("The limit must be between 1 and 50.".into());
        }
        if self.query.chars().filter(|c| *c == '"').count() % 2 != 0 {
            return Err("Close the quoted phrase with a double quote.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub excerpt: String,
}

#[derive(Debug, PartialEq, Eq)]
struct Record {
    path: String,
    title: String,
    headings: Vec<String>,
    body: String,
}

pub struct SearchIndex {
    records: Vec<Record>,
    aliases: BTreeMap<String, usize>,
    reader: IndexReader,
    fields: [Field; 3],
    id: Field,
}

impl std::fmt::Debug for SearchIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchIndex")
            .field("pages", &self.records.len())
            .finish()
    }
}

fn analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(LowerCaser)
        .build()
}

// Keep adjacent inline text intact, but separate block elements and omit non-content subtrees.
fn extract_text(element: scraper::ElementRef<'_>, body: &mut String, headings: &mut Vec<String>) {
    let el = element.value();
    if matches!(el.name(), "script" | "style" | "nav")
        || el.attr("hidden").is_some()
        || el.attr("aria-hidden") == Some("true")
        || el.has_class("skill-download", scraper::CaseSensitivity::CaseSensitive)
        || el
            .attr("style")
            .is_some_and(|s| s.replace(' ', "").to_lowercase().contains("display:none"))
    {
        return;
    }
    let heading = matches!(el.name(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
    let block = heading
        || matches!(
            el.name(),
            "p" | "div"
                | "pre"
                | "blockquote"
                | "li"
                | "ul"
                | "ol"
                | "dl"
                | "dt"
                | "dd"
                | "table"
                | "tr"
                | "td"
                | "th"
                | "section"
                | "article"
                | "header"
                | "footer"
                | "aside"
                | "br"
                | "hr"
        );
    if block {
        body.push(' ');
    }
    let start = body.len();
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            body.push_str(text);
        } else if let Some(child) = scraper::ElementRef::wrap(child) {
            extract_text(child, body, headings);
        }
    }
    if heading {
        headings.push(
            body[start..]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    if block {
        body.push(' ');
    }
}

/// Build only from published page routes. Sources and directory aliases share a record.
pub fn build(
    site: &Site,
    routes: &BTreeMap<String, Route>,
    previous: Option<&Arc<SearchIndex>>,
) -> Result<Arc<SearchIndex>, String> {
    let mut canonical = BTreeMap::new();
    for (path, route) in routes {
        if !matches!(route, Route::Page { .. }) {
            continue;
        }
        canonical.insert(path.clone(), path.clone());
    }
    for dir in site.dirs.iter().map(|dir| {
        if dir.is_empty() {
            "/".to_owned()
        } else {
            format!("/{dir}/")
        }
    }) {
        if !canonical.contains_key(&dir) {
            continue;
        }
        if let Some(source) = [format!("{dir}index.md"), format!("{dir}README.md")]
            .into_iter()
            .find(|p| site.pages.contains_key(p))
        {
            canonical.insert(source, dir);
        }
    }
    let mut records = Vec::new();
    let mut ids = BTreeMap::new();
    let main = Selector::parse("main#main").unwrap();
    let title = Selector::parse("meta[property='og:title']").unwrap();
    let description = Selector::parse("meta[name='description']").unwrap();
    for (path, target) in &canonical {
        if path != target {
            continue;
        }
        let Route::Page { html, .. } = &routes[path] else {
            unreachable!()
        };
        let document = Html::parse_document(html);
        let title = document
            .select(&title)
            .next()
            .and_then(|el| el.value().attr("content"))
            .unwrap_or(path)
            .to_owned();
        let mut body = document
            .select(&description)
            .next()
            .and_then(|el| el.value().attr("content"))
            .unwrap_or("")
            .to_owned();
        let mut headings = Vec::new();
        if let Some(main) = document.select(&main).next() {
            body.push(' ');
            extract_text(main, &mut body, &mut headings);
        }
        let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
        ids.insert(path.clone(), records.len());
        records.push(Record {
            path: path.clone(),
            title,
            headings,
            body,
        });
    }
    let mut aliases: BTreeMap<String, usize> = canonical
        .into_iter()
        .map(|(alias, target)| (alias, ids[&target]))
        .collect();
    for (path, route) in routes {
        if let Route::Redirect(target) = route
            && let Some(id) = aliases.get(target).copied()
        {
            aliases.insert(path.clone(), id);
        }
    }
    if let Some(previous) = previous
        && previous.records == records
        && previous.aliases == aliases
    {
        return Ok(previous.clone());
    }
    let mut schema = Schema::builder();
    let text = TextOptions::default().set_stored().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("docs")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    let fields = [
        schema.add_text_field("title", text.clone()),
        schema.add_text_field("headings", text.clone()),
        schema.add_text_field("body", text),
    ];
    let id = schema.add_u64_field("id", tantivy::schema::STORED);
    let index = Index::create_in_ram(schema.build());
    index.tokenizers().register("docs", analyzer());
    let mut writer = index
        .writer_with_num_threads::<TantivyDocument>(1, 15_000_000)
        .map_err(|e| e.to_string())?;
    for (i, record) in records.iter().enumerate() {
        let mut document = tantivy::doc!(id => i as u64, fields[0] => record.title.clone(), fields[2] => record.body.clone());
        // Tantivy inserts a position gap between field values, so phrases cannot join headings.
        for heading in &record.headings {
            document.add_text(fields[1], heading);
        }
        writer.add_document(document).map_err(|e| e.to_string())?;
    }
    writer.commit().map_err(|e| e.to_string())?;
    writer.wait_merging_threads().map_err(|e| e.to_string())?;
    let reader = index.reader().map_err(|e| e.to_string())?;
    Ok(Arc::new(SearchIndex {
        records,
        aliases,
        reader,
        fields,
        id,
    }))
}

impl SearchIndex {
    pub fn page(&self, path: &str) -> Option<(&str, &str)> {
        let record = &self.records[*self.aliases.get(path)?];
        Some((&record.path, &record.title))
    }

    pub fn search(&self, request: &SearchRequest) -> Result<SearchResponse, String> {
        request.validate()?;
        let mut clauses = Vec::new();
        let mut tokenizer = analyzer();
        for (i, part) in request.query.split('"').enumerate() {
            let mut words = Vec::new();
            tokenizer
                .token_stream(part)
                .process(&mut |token| words.push(token.text.clone()));
            if i % 2 == 1 {
                if !words.is_empty() {
                    clauses.push(words);
                }
            } else {
                clauses.extend(words.into_iter().map(|word| vec![word]));
            }
        }
        if clauses.is_empty() || self.records.is_empty() {
            return Ok(SearchResponse {
                results: vec![],
                total: 0,
            });
        }
        let query = BooleanQuery::new(
            clauses
                .into_iter()
                .map(|words| {
                    let fields = self
                        .fields
                        .iter()
                        .zip([5.0, 2.0, 1.0])
                        .map(|(&field, boost)| {
                            let terms: Vec<_> = words
                                .iter()
                                .map(|word| Term::from_field_text(field, word))
                                .collect();
                            let query: Box<dyn Query> = if terms.len() == 1 {
                                Box::new(TermQuery::new(
                                    terms[0].clone(),
                                    IndexRecordOption::WithFreqsAndPositions,
                                ))
                            } else {
                                Box::new(PhraseQuery::new(terms))
                            };
                            (
                                Occur::Should,
                                Box::new(BoostQuery::new(query, boost)) as Box<dyn Query>,
                            )
                        })
                        .collect();
                    (
                        Occur::Must,
                        Box::new(BooleanQuery::new(fields)) as Box<dyn Query>,
                    )
                })
                .collect(),
        );
        let searcher = self.reader.searcher();
        // Collect at most one hit per page, then apply stable URL tie-breaking before pagination.
        let hits = searcher
            .search(
                &query,
                &TopDocs::with_limit(self.records.len()).order_by_score(),
            )
            .map_err(|e| e.to_string())?;
        let mut hits = hits
            .into_iter()
            .map(|(score, address)| {
                use tantivy::schema::Value;
                let doc: TantivyDocument = searcher.doc(address).map_err(|e| e.to_string())?;
                let id = doc
                    .get_first(self.id)
                    .and_then(|v| v.as_u64())
                    .ok_or("Missing search record ID")? as usize;
                Ok((score, id, doc))
            })
            .collect::<Result<Vec<_>, String>>()?;
        hits.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| self.records[a.1].path.cmp(&self.records[b.1].path))
        });
        let total = hits.len();
        let mut snippets = SnippetGenerator::create(&searcher, &query, self.fields[2])
            .map_err(|e| e.to_string())?;
        snippets.set_max_num_chars(240);
        let results = hits
            .into_iter()
            .skip(request.offset)
            .take(request.limit)
            .map(|(_, id, doc)| {
                let record = &self.records[id];
                let snippet = snippets.snippet_from_doc(&doc);
                let excerpt = if snippet.fragment().is_empty() {
                    record.body.chars().take(240).collect()
                } else {
                    snippet.fragment().to_owned()
                };
                SearchResult {
                    title: record.title.clone(),
                    url: render::url(&record.path),
                    excerpt,
                }
            })
            .collect();
        Ok(SearchResponse { results, total })
    }
}

pub fn results_html(request: &SearchRequest, response: &SearchResponse) -> maud::Markup {
    let link = |offset: usize| {
        let query = serde_urlencoded::to_string([
            ("q", request.query.clone()),
            ("limit", request.limit.to_string()),
            ("offset", offset.to_string()),
        ])
        .unwrap();
        format!("{HTML_PATH}?{query}")
    };
    maud::html! {
        h1 { "Search" }
        form action=(HTML_PATH) method="get" {
            label for="results-query" { "Search documentation" }
            " " input id="results-query" type="search" name="q" value=(&request.query) maxlength="1024";
            button { "Search" }
        }
        @if request.query.trim().is_empty() {
            p { "Enter words or a quoted phrase." }
        } @else {
            p { (response.total) " results" }
            ol class="search-results" {
                @for result in &response.results {
                    li { a href=(&result.url) { (&result.title) } p { (&result.excerpt) } }
                }
            }
            nav aria-label="Search results pages" {
                @if request.offset > 0 { a href=(link(request.offset.saturating_sub(request.limit))) { "Previous" } " " }
                @if let Some(next) = request.offset.checked_add(request.limit).filter(|next| *next < response.total) {
                    a href=(link(next)) { "Next" }
                }
            }
        }
    }
}
