// The page's motion: Lenis for the smooth scroll, GSAP and ScrollTrigger for
// the rest, on one clock (GSAP's ticker drives Lenis). Everything here only
// adds movement: the page reads the same without it, and with "reduce
// motion" none of it runs (the head script then leaves out the .motion class
// that hides what is about to come in).
import Lenis from 'lenis';
import 'lenis/dist/lenis.css';
import { gsap } from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';

gsap.registerPlugin(ScrollTrigger);

const root = document.documentElement;
const MOTION = '(prefers-reduced-motion: no-preference)';
const $ = <T extends Element = HTMLElement>(sel: string, scope: ParentNode = document) => Array.from(scope.querySelectorAll<T>(sel));

root.classList.add('motion-ready');

if (matchMedia(MOTION).matches) {
  const header = parseFloat(getComputedStyle(root).getPropertyValue('--header-h')) || 60;
  const lenis = new Lenis({ autoRaf: false, lerp: 0.11, anchors: { offset: -(header + 16) } });
  lenis.on('scroll', ScrollTrigger.update);
  gsap.ticker.add((time) => lenis.raf(time * 1000));
  gsap.ticker.lagSmoothing(0);
}

const mm = gsap.matchMedia();

// Created first, so the triggers further down are measured with its pin.
mm.add(`${MOTION} and (min-width: 960px)`, () => stage());
mm.add(MOTION, () => {
  hero();
  rails();
  reveals();
  shots();
  log();
  wipe();
  phones();
  terminal();
  headerAway();
});

// Images arrive late and move everything below them.
window.addEventListener('load', () => ScrollTrigger.refresh());

/* ------------------------------------------------------------ hero */

function hero() {
  const words = $('.hero h1 .w > span');
  const gather = $('.hero .gather path');
  const tl = gsap.timeline({ defaults: { ease: 'expo.out' } });
  tl.set('.hero .gather', { opacity: 1 })
    .fromTo(gather, { strokeDasharray: 1, strokeDashoffset: 1 }, { strokeDashoffset: 0, duration: 1.1, stagger: 0.12, ease: 'power2.inOut' }, 0)
    .fromTo('.hero .rail [data-node]', { scale: 0 }, { scale: 1, duration: 0.6, ease: 'back.out(3)' }, 0.75)
    .fromTo(words, { y: 0, yPercent: 108 }, { y: 0, yPercent: 0, duration: 1.1, stagger: 0.055 }, 0.15)
    .fromTo('.hero [data-in]', { autoAlpha: 0, y: 18 }, { autoAlpha: 1, y: 0, duration: 0.9, stagger: 0.08 }, 0.55)
    .fromTo('[data-hero-shot]', { autoAlpha: 0, y: 60 }, { autoAlpha: 1, y: 0, duration: 1.4 }, 0.7);

  // The window lies back, then stands up as it comes into view.
  gsap.fromTo(
    '[data-hero-shot] .shot',
    { rotateX: 16, scale: 0.94 },
    {
      rotateX: 0,
      scale: 1,
      ease: 'none',
      scrollTrigger: { trigger: '.hero-stage', start: 'top 85%', end: 'top 25%', scrub: true },
    },
  );
}

/* ------------------------------------------------------------ the history's lanes */

function rails() {
  for (const rail of $('[data-rail]')) {
    const section = rail.closest('section')!;
    if (section.matches('[data-stage].pinned')) continue; // drawn by its own timeline
    const trunk = rail.querySelector('[data-trunk]');
    const branch = rail.querySelector('[data-branch]');
    const curves = $<SVGPathElement>('[data-curve] path', rail);
    const node = rail.querySelector('[data-node]');
    const isHero = section.classList.contains('hero');

    gsap.set(trunk, { opacity: 1 });
    const tl = gsap.timeline({
      defaults: { ease: 'none' },
      scrollTrigger: { trigger: section, start: 'top 70%', end: 'bottom 70%', scrub: 0.6 },
    });
    tl.fromTo(trunk, { scaleY: 0 }, { scaleY: 1, duration: 1 }, 0);
    if (curves.length === 2) {
      tl.fromTo(curves[0], { strokeDasharray: 1, strokeDashoffset: 1 }, { strokeDashoffset: 0, duration: 0.08 }, 0)
        .fromTo(branch, { scaleY: 0 }, { scaleY: 1, duration: 0.84 }, 0.08)
        .fromTo(curves[1], { strokeDasharray: 1, strokeDashoffset: 1 }, { strokeDashoffset: 0, duration: 0.08 }, 0.92);
    }
    if (node && !isHero) {
      gsap.fromTo(node, { scale: 0 }, { scale: 1, duration: 0.5, ease: 'back.out(3)', scrollTrigger: { trigger: section, start: 'top 72%', toggleActions: 'play none none reverse' } });
    }
  }
}

/* ------------------------------------------------------------ text and blocks coming in */

function reveals() {
  for (const section of $('section.commit:not(.hero)')) {
    const items = $('[data-in]', section).filter((el) => !el.closest('.stage.pinned'));
    if (!items.length) continue;
    gsap.fromTo(
      items,
      { autoAlpha: 0, y: 26 },
      { autoAlpha: 1, y: 0, duration: 0.9, ease: 'expo.out', stagger: 0.07, scrollTrigger: { trigger: section, start: 'top 72%', once: true } },
    );
  }
}

function shots() {
  for (const shot of $('[data-shot]')) {
    const imgs = $('img', shot);
    const tl = gsap.timeline({ scrollTrigger: { trigger: shot, start: 'top 92%', end: 'top 35%', scrub: 0.5 } });
    tl.fromTo(shot, { clipPath: 'inset(7% 7% 7% 7% round 22px)' }, { clipPath: 'inset(0% 0% 0% 0% round 14px)', ease: 'none' }, 0).fromTo(
      imgs,
      { scale: 1.12 },
      { scale: 1, ease: 'none' },
      0,
    );
  }
}

function log() {
  for (const row of $('[data-log-row]')) {
    const tl = gsap.timeline({ scrollTrigger: { trigger: row, start: 'top 82%', once: true } });
    tl.fromTo($('.log-graph i', row), { scale: 0 }, { scale: 1, duration: 0.5, ease: 'back.out(3)' }, 0).fromTo(
      row.lastElementChild,
      { autoAlpha: 0, x: -16 },
      { autoAlpha: 1, x: 0, duration: 0.8, ease: 'expo.out' },
      0.05,
    );
  }
}

/* ------------------------------------------------------------ dark into light */

function wipe() {
  for (const el of $('[data-wipe]')) {
    gsap.fromTo(el, { '--cut': '94%' }, { '--cut': '6%', ease: 'none', scrollTrigger: { trigger: el, start: 'top 80%', end: 'bottom 30%', scrub: 0.6 } });
  }
}

/* ------------------------------------------------------------ the three phones fan out */

function phones() {
  const wrap = document.querySelector('[data-phones]');
  if (!wrap) return;
  const [a, b, c] = $('.phone', wrap);
  const tl = gsap.timeline({ defaults: { ease: 'none' }, scrollTrigger: { trigger: wrap, start: 'top 90%', end: 'center 50%', scrub: 0.6 } });
  tl.fromTo(a, { xPercent: 70, rotate: -8, y: 40 }, { xPercent: 0, rotate: 0, y: 0 }, 0)
    .fromTo(c, { xPercent: -70, rotate: 8, y: 40 }, { xPercent: 0, rotate: 0, y: 0 }, 0)
    .fromTo(b, { y: 60, scale: 0.94 }, { y: 0, scale: 1 }, 0);
}

/* ------------------------------------------------------------ the build typed out */

function terminal() {
  const term = document.querySelector('[data-terminal]');
  if (!term) return;
  const cmds = $('[data-cmd]', term);
  const outs = $('[data-out]', term);
  const full = cmds.map((c) => c.textContent ?? '');
  const lines = cmds.map((c) => c.closest('.line')!);
  const caret = term.querySelector('.caret')?.closest('.line');

  gsap.set([...lines, ...outs, caret], { autoAlpha: 0 });
  const tl = gsap.timeline({ paused: true });
  cmds.forEach((cmd, i) => {
    const typed = { n: 0 };
    tl.set(lines[i], { autoAlpha: 1 })
      .set(cmd, { textContent: '' })
      .to(typed, {
        n: full[i].length,
        duration: full[i].length * 0.022,
        ease: 'none',
        onUpdate: () => (cmd.textContent = full[i].slice(0, Math.round(typed.n))),
      })
      .to({}, { duration: 0.25 });
  });
  tl.to(outs, { autoAlpha: 1, duration: 0.2, stagger: 0.35 }).set(caret, { autoAlpha: 1 });

  ScrollTrigger.create({ trigger: term, start: 'top 75%', once: true, onEnter: () => tl.play() });
  // Copy and a screen reader always get the whole commands.
  return () => {
    tl.kill();
    cmds.forEach((c, i) => (c.textContent = full[i]));
  };
}

/* ------------------------------------------------------------ board, then city, pinned */

function stage() {
  const section = document.querySelector<HTMLElement>('[data-stage]');
  if (!section) return;
  section.classList.add('pinned');
  gsap.set($('[data-in]', section), { autoAlpha: 1 });
  const [first, second] = $('[data-step]', section);
  const [t1, t2] = [first, second].map((s) => s.querySelector('.stage-text')!);
  const [s1, s2] = [first, second].map((s) => s.querySelector('.shot')!);
  const rail = section.querySelector('[data-rail]')!;
  const curves = $<SVGPathElement>('[data-curve] path', rail);

  gsap.set(rail.querySelector('[data-trunk]'), { opacity: 1 });
  const tl = gsap.timeline({
    defaults: { ease: 'none' },
    scrollTrigger: { trigger: section, start: 'top top', end: '+=140%', pin: true, scrub: 0.6, anticipatePin: 1 },
  });
  tl.fromTo(rail.querySelector('[data-trunk]'), { scaleY: 0 }, { scaleY: 1, duration: 1 }, 0)
    .fromTo(curves[0], { strokeDasharray: 1, strokeDashoffset: 1 }, { strokeDashoffset: 0, duration: 0.1 }, 0)
    .fromTo(rail.querySelector('[data-branch]'), { scaleY: 0 }, { scaleY: 1, duration: 0.8 }, 0.1)
    .fromTo(curves[1], { strokeDasharray: 1, strokeDashoffset: 1 }, { strokeDashoffset: 0, duration: 0.1 }, 0.9)
    // The board, then the city rising over it from below.
    .fromTo(s2, { clipPath: 'inset(100% 0% 0% 0% round 14px)' }, { clipPath: 'inset(0% 0% 0% 0% round 14px)', duration: 0.45, ease: 'power2.inOut' }, 0.35)
    .fromTo(s1, { scale: 1 }, { scale: 0.94, autoAlpha: 0.4, duration: 0.45 }, 0.35)
    .fromTo(t1, { autoAlpha: 1, y: 0 }, { autoAlpha: 0, y: -30, duration: 0.2 }, 0.35)
    .fromTo(t2, { autoAlpha: 0, y: 30 }, { autoAlpha: 1, y: 0, duration: 0.25 }, 0.55);

  return () => section.classList.remove('pinned');
}

/* ------------------------------------------------------------ the header steps aside */

function headerAway() {
  const top = document.querySelector('.top');
  if (!top) return;
  ScrollTrigger.create({
    start: 0,
    end: 'max',
    onUpdate: (self) => top.classList.toggle('away', self.direction === 1 && self.scroll() > 400),
  });
  return () => top.classList.remove('away');
}
