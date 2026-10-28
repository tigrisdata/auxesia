(() => {
  const form = document.querySelector('#site-search');
  const dialog = document.querySelector('#search-dialog');
  if (!form || !dialog || typeof dialog.showModal !== 'function') return;
  const input = dialog.querySelector('#dialog-query');
  const status = dialog.querySelector('#search-status');
  const results = dialog.querySelector('.search-results');
  const all = dialog.querySelector('#search-all');
  let previousFocus;
  let timer;
  let controller;
  let revision = 0;

  function invalidate() {
    clearTimeout(timer);
    controller?.abort();
    revision += 1;
  }

  function schedule() {
    invalidate();
    const current = revision;
    const query = input.value;
    all.href = `/_auxesia/search?${new URLSearchParams({ q: query })}`;
    results.replaceChildren();
    if (!query.trim()) {
      status.textContent = 'Enter words or a quoted phrase.';
      return;
    }
    status.textContent = 'Searching…';
    timer = setTimeout(async () => {
      controller = new AbortController();
      try {
        const response = await fetch(`/_auxesia/search.json?${new URLSearchParams({ q: query })}`, { signal: controller.signal });
        const data = await response.json();
        if (current !== revision || !dialog.open) return;
        if (!response.ok) throw new Error(data.error || 'Search failed. Try again.');
        const items = data.results.map(result => {
          const item = document.createElement('li');
          const link = document.createElement('a');
          link.href = result.url;
          link.textContent = result.title;
          const excerpt = document.createElement('p');
          excerpt.textContent = result.excerpt;
          item.append(link, excerpt);
          return item;
        });
        results.replaceChildren(...items);
        status.textContent = data.total ? `${data.total} results` : 'No results.';
      } catch (error) {
        if (current !== revision || error.name === 'AbortError' || !dialog.open) return;
        status.textContent = error.message || 'Search failed. Try again.';
      }
    }, 200);
  }

  function open() {
    if (dialog.open) return;
    previousFocus = document.activeElement;
    input.value = form.elements.q.value;
    dialog.showModal();
    input.focus();
    schedule();
  }

  form.addEventListener('submit', event => { event.preventDefault(); open(); });
  document.addEventListener('keydown', event => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      open();
    }
  });
  input.addEventListener('input', schedule);
  dialog.querySelector('#search-close').addEventListener('click', () => dialog.close());
  dialog.addEventListener('close', () => {
    invalidate();
    form.elements.q.value = input.value;
    previousFocus?.focus();
  });
  dialog.addEventListener('keydown', event => {
    if (event.key === 'Escape') {
      event.preventDefault();
      dialog.close();
      return;
    }
    if (event.key !== 'Tab') return;
    const focusable = [...dialog.querySelectorAll('input, button, a[href]')];
    const first = focusable[0];
    const last = focusable.at(-1);
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault(); last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault(); first.focus();
    }
  });
})();
