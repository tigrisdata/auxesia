# Response checks

These commands use `http://127.0.0.1:8080` and the documentation bundled in `ghcr.io/tigrisdata/auxesia`.
Run them while the container serves its bundled `/docs` folder, without a replacement mount.
No source checkout is required.
If you changed the hostname or port, adjust each URL and the Python `base` value.
For other documentation, select page and skill URLs from `/llms.txt`.
Use the selected skill name in the archive paths and expected entries.

## HTML

Request the skill page:

```sh
curl --fail --silent --show-error -D - http://127.0.0.1:8080/skills/install-auxesia
```

The response has status `200` and a `text/html` content type.
The body includes the skill instructions and a `.skill` download link.
Relative links in the body start with `/skills/install-auxesia/`. Fragment links such as `#install` stay on the page.

## Original Markdown

Request the same page as Markdown:

```sh
curl --fail --silent --show-error \
  -H 'Accept: text/markdown' \
  http://127.0.0.1:8080/skills/install-auxesia
```

The body starts with the original YAML frontmatter.
The body includes `name: install-auxesia` and the relative link `references/checks.md`.
It contains the original source bytes, rather than the HTML page.

## Agent discovery

Request the discovery document:

```sh
curl --fail --silent --show-error http://127.0.0.1:8080/llms.txt
```

The response includes documentation and skill sections.
The skill section lists `/skills/install-auxesia` and `/skills/install-auxesia.skill`.
The document explains Markdown requests and states that Auxesia does not run skills.

## Supporting file

Request the supporting reference:

```sh
curl --fail --silent --show-error \
  http://127.0.0.1:8080/skills/install-auxesia/references/checks.md
```

The response contains this Markdown source without an HTML wrapper.
The bundle entrypoint links to this file relative to the bundle folder.

## Download and archive

Download the archive to a temporary directory:

```sh
skill_check_dir=$(mktemp -d)
curl --fail --silent --show-error \
  -o "$skill_check_dir/install-auxesia.skill" \
  http://127.0.0.1:8080/skills/install-auxesia.skill
```

Inspect its entries and compare them with the source served by the same container:

```sh
python3 - "$skill_check_dir/install-auxesia.skill" <<'PY'
import sys
import zipfile
from urllib.request import Request, urlopen

base = "http://127.0.0.1:8080"
sources = {
    "install-auxesia/SKILL.md": "/skills/install-auxesia",
    "install-auxesia/references/checks.md": "/skills/install-auxesia/references/checks.md",
}

with zipfile.ZipFile(sys.argv[1]) as archive:
    names = archive.namelist()
    assert names == [
        "install-auxesia/SKILL.md",
        "install-auxesia/references/checks.md",
    ], names
    for name in names:
        request = Request(base + sources[name], headers={"Accept": "text/markdown"})
        with urlopen(request, timeout=10) as response:
            assert archive.read(name) == response.read(), name
        print(name)
print("Archive entries match the served source files.")
PY
```

The command prints both entries and `Archive entries match the served source files.`.
Each archive entry starts with `install-auxesia/`.
An agent can read the instructions and supporting file after extraction.
