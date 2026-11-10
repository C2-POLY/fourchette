// The Undo demo (components/UndoDemo.astro). Each action changes the small
// history and pushes the step that takes it back, as the app's Snapshot does;
// Undo pops the last one. Nothing leaves the page.
import { gsap } from 'gsap';

type Step = { label: string; back: () => void };

const SUBJECTS = ['Tidy the toolbar icons', 'Explain the undo stack', 'Round the merge curves', 'Name the stash after its branch'];
const MAX = 30;

export function mountUndoDemo(root: HTMLElement) {
  const log = root.querySelector<HTMLOListElement>('[data-log]')!;
  const count = root.querySelector<HTMLElement>('[data-count]')!;
  const toast = root.querySelector<HTMLElement>('[data-toast]')!;
  const button = (act: string) => root.querySelector<HTMLButtonElement>(`[data-act="${act}"]`)!;
  const label = (act: string) => button(act).textContent!.trim();
  const still = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

  let wip: HTMLLIElement | null = log.querySelector<HTMLLIElement>('[data-wip]');
  const stack: Step[] = [];
  let made = 0;
  let toastTimer = 0;

  const enter = (el: HTMLElement) => {
    if (still()) return;
    gsap.fromTo(el, { autoAlpha: 0, y: -10 }, { autoAlpha: 1, y: 0, duration: 0.35, ease: 'power3.out', clearProps: 'all' });
  };
  const leave = (el: HTMLElement, done: () => void) => {
    if (still()) return done();
    gsap.to(el, { autoAlpha: 0, x: 14, duration: 0.2, ease: 'power2.in', onComplete: () => (done(), gsap.set(el, { clearProps: 'all' })) });
  };
  const flash = (el: Element) => {
    el.classList.remove('flash');
    void (el as HTMLElement).offsetWidth;
    el.classList.add('flash');
  };

  const say = (text: string) => {
    toast.textContent = text;
    toast.classList.add('on');
    clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => toast.classList.remove('on'), 2600);
  };

  const refresh = () => {
    const attached = !!wip?.isConnected;
    button('commit').disabled = !attached;
    button('stash').disabled = !attached;
    button('delete').disabled = !log.querySelector('[data-branch]');
    button('undo').disabled = stack.length === 0;
    count.textContent = String(stack.length);
  };

  const push = (step: Step) => {
    stack.push(step);
    if (stack.length > MAX) stack.shift();
    refresh();
  };

  const actions: Record<string, () => void> = {
    commit() {
      if (!wip?.isConnected) return;
      const changes = wip;
      const head = log.querySelector<HTMLElement>('[data-head]')!;
      const from = head.parentElement!;
      const row = document.createElement('li');
      row.className = 'row';
      row.style.setProperty('--lane', 'var(--lane-1)');
      row.innerHTML = '<span class="g"><i class="dot"></i></span><span class="msg"></span>';
      row.querySelector('.msg')!.textContent = SUBJECTS[made++ % SUBJECTS.length];
      changes.replaceWith(row);
      row.append(head);
      enter(row);
      push({
        label: label('commit'),
        back() {
          from.append(head);
          leave(row, () => {
            row.replaceWith(changes);
            enter(changes);
            flash(changes);
            refresh();
          });
        },
      });
    },
    delete() {
      const pill = log.querySelector<HTMLElement>('[data-branch]');
      if (!pill) return;
      const parent = pill.parentElement!;
      const name = pill.dataset.branch!;
      const at = pill.nextSibling;
      leave(pill, () => (pill.remove(), refresh()));
      pill.removeAttribute('data-branch');
      push({
        label: `${label('delete')} ${name}`,
        back() {
          pill.dataset.branch = name;
          parent.insertBefore(pill, at);
          enter(pill);
          flash(pill);
        },
      });
    },
    stash() {
      if (!wip?.isConnected) return;
      const changes = wip;
      const next = changes.nextSibling;
      leave(changes, () => (changes.remove(), refresh()));
      push({
        label: label('stash'),
        back() {
          log.insertBefore(changes, next);
          enter(changes);
          flash(changes);
        },
      });
    },
    undo() {
      const step = stack.pop();
      if (!step) return say(root.dataset.none!);
      step.back();
      say(`${root.dataset.took} · ${step.label}`);
      refresh();
      if (!still()) gsap.fromTo(button('undo'), { scale: 0.94 }, { scale: 1, duration: 0.3, ease: 'back.out(3)' });
    },
  };

  root.addEventListener('click', (e) => {
    const act = (e.target as Element).closest<HTMLButtonElement>('[data-act]')?.dataset.act;
    if (act) actions[act]();
  });

  // The visitor's own ⌘Z (Ctrl+Z elsewhere) while the demo is on screen,
  // unless they are typing somewhere.
  let seen = false;
  new IntersectionObserver(([e]) => (seen = e.isIntersecting), { threshold: 0.4 }).observe(root);
  document.addEventListener('keydown', (e) => {
    if (!seen || e.shiftKey || e.altKey || !(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== 'z') return;
    const t = e.target as HTMLElement;
    if (t.closest('input, textarea, [contenteditable]')) return;
    e.preventDefault();
    actions.undo();
  });

  refresh();
}
