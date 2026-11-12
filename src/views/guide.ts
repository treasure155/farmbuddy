import { t } from '../lib/i18n';
import { $, $$, debounce, esc } from '../lib/dom';
import { icon } from '../lib/icons';
import { GUIDE } from './guide-content';
import type { ViewCtx } from './shell';

let current = GUIDE[0].id;

export function render(ctx: ViewCtx) {
  ctx.header(t('page.guide.title', {}, 'How to'), t('page.guide.subtitle', {}, 'The FarmBuddy user guide. It works offline.'), `<button class="btn btn-ghost" data-print-guide>${icon('print', 16)} Print guide</button>`);
  ctx.main.innerHTML = `<div class="guide">
    <nav class="guide-nav" aria-label="Guide sections">
      <div class="search-field">${icon('search', 16)}<input type="search" data-gsearch placeholder="Search the guide" aria-label="Search the guide"></div>
      <ol data-toc>${GUIDE.map((g) => `<li><button data-sec="${g.id}" class="${g.id === current ? 'is-on' : ''}">${esc(g.title)}</button></li>`).join('')}</ol>
    </nav>
    <article class="guide-body" data-body></article>
  </div>`;
  const body = $('[data-body]', ctx.main)!;

  const showSection = (id: string) => {
    current = id;
    const g = GUIDE.find((x) => x.id === id) ?? GUIDE[0];
    const i = GUIDE.indexOf(g);
    body.innerHTML = `<header class="guide-head"><h2>${esc(g.title)}</h2>${g.who ? `<span class="pill pill-info">${esc(g.who)}</span>` : ''}</header>
      <div class="guide-text">${g.html}</div>
      <footer class="guide-pager">
        ${i > 0 ? `<button class="btn btn-ghost" data-sec="${GUIDE[i - 1].id}">${icon('back', 16)} ${esc(GUIDE[i - 1].title)}</button>` : '<span></span>'}
        ${i < GUIDE.length - 1 ? `<button class="btn btn-ghost" data-sec="${GUIDE[i + 1].id}">${esc(GUIDE[i + 1].title)} ${icon('arrowRight', 16)}</button>` : ''}
      </footer>`;
    $$<HTMLElement>('[data-toc] [data-sec]', ctx.main).forEach((b) => b.classList.toggle('is-on', b.dataset.sec === id));
    ctx.main.scrollTop = 0;
  };

  const showResults = (q: string) => {
    const needle = q.trim().toLowerCase();
    if (!needle) { showSection(current); return; }
    const hits = GUIDE.map((g) => {
      const text = g.html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ');
      const at = text.toLowerCase().indexOf(needle);
      return { g, at, text, titleHit: g.title.toLowerCase().includes(needle) };
    }).filter((h) => h.at >= 0 || h.titleHit);
    body.innerHTML = `<header class="guide-head"><h2>Search: “${esc(q.trim())}”</h2></header>
      ${hits.length ? `<ul class="guide-hits">${hits.map((h) => {
        const start = Math.max(0, h.at - 60);
        const snippet = h.at >= 0 ? `${start ? '…' : ''}${esc(h.text.slice(start, h.at + needle.length + 90))}…` : '';
        return `<li><button data-sec="${h.g.id}"><b>${esc(h.g.title)}</b><span>${snippet}</span></button></li>`;
      }).join('')}</ul>` : '<p class="muted">Nothing found. Try a shorter word, such as “barcode”, “password” or “print”.</p>'}`;
  };

  showSection(current);
  ctx.main.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-sec]');
    if (b) { ($('[data-gsearch]', ctx.main) as HTMLInputElement).value = ''; showSection(b.dataset.sec!); }
  });
  $('[data-gsearch]', ctx.main)!.addEventListener('input', debounce((e: Event) => showResults((e.target as HTMLInputElement).value), 150));
  $('[data-print-guide]')?.addEventListener('click', async () => {
    const { printHtml } = await import('../lib/receipt');
    printHtml(`<article class="guide-print"><h1>FarmBuddy user guide</h1>${GUIDE.map((g) => `<section><h2>${esc(g.title)}</h2>${g.html}</section>`).join('')}</article>`);
  });
}
