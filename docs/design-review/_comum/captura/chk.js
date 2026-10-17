window.__run = async (state) => {
  const w = (ms) => new Promise((r) => setTimeout(r, ms));
  const click = (re) =>
    [...document.querySelectorAll('button')]
      .find((b) => re.test(b.textContent + (b.getAttribute('aria-label') || '')))
      ?.click();
  if (state === 'grid') {
    click(/^Grade/);
    await w(400);
  }
  if (state === 'details') {
    document.querySelector('button[aria-label^="Detalhes de"]')?.click();
    await w(500);
  }
  if (state === 'inspector') {
    [...document.querySelectorAll('[role=tab]')]
      .find((b) => b.textContent.startsWith('Execuç'))
      ?.click();
    await w(300);
    document.querySelector('[role=tabpanel] button')?.click();
    await w(500);
  }
  if (state === 'projects' || state === 'analyze' || state === 'review') {
    click(/Projetos \(/);
    await w(700);
  }
  if (state === 'analyze' || state === 'review') {
    click(/Atualizar/);
    await w(1000);
  }
  if (state === 'review') {
    click(/Atualizar o existente/);
    await w(700);
  }
  if (state === 'switcher') {
    document.querySelector('[aria-haspopup=listbox]')?.click();
    await w(300);
  }
  const de = document.documentElement,
    vw = de.clientWidth,
    bad = [];
  document.querySelectorAll('body *').forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.width > 0 && r.right > vw + 1) {
      let p = el.parentElement,
        c = false;
      while (p) {
        const o = getComputedStyle(p).overflowX;
        if (o !== 'visible') {
          const pr = p.getBoundingClientRect();
          if (pr.right <= vw + 1) {
            c = true;
            break;
          }
        }
        p = p.parentElement;
      }
      if (!c)
        bad.push((el.className || el.tagName).toString().slice(0, 40) + ':' + Math.round(r.right));
    }
  });
  const trunc = [];
  document.querySelectorAll('body *').forEach((el) => {
    const cs = getComputedStyle(el);
    if (
      el.children.length === 0 &&
      el.clientWidth > 0 &&
      el.scrollWidth > el.clientWidth + 1 &&
      cs.overflowX === 'visible' &&
      cs.display !== 'inline'
    )
      trunc.push((el.className || el.tagName).toString().slice(0, 40));
  });
  return JSON.stringify({
    s: state,
    vw,
    sw: de.scrollWidth,
    bad: bad.slice(0, 5),
    trunc: trunc.slice(0, 5),
  });
};
{
  const auto = new URLSearchParams(location.search).get('s');
  if (auto) window.addEventListener('load', () => setTimeout(() => void window.__run(auto), 2500));
}
