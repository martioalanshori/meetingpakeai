<script lang="ts">
  import { untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import { endTour, tour, TOUR_STEPS } from "$lib/tour.svelte";

  const t = id.guide;
  const total = TOUR_STEPS.length;
  /** Jarak sorotan dari tepi elemen dan jarak kartu dari sorotan (px). */
  const PAD = 6;
  const GAP = 14;
  const EDGE = 16;

  type Box = { top: number; left: number; width: number; height: number };

  let el = $state<HTMLElement | null>(null);
  let box = $state<Box | null>(null);
  let ready = $state(false);
  let vw = $state(0);
  let vh = $state(0);
  let cardH = $state(160);
  let dir = 1;
  let token = 0;

  const step = $derived(TOUR_STEPS[Math.min(tour.index, total - 1)]);
  const text = $derived(t.steps[step.key] as { title: string; text: string; tryIt?: string });
  const last = $derived(tour.index >= total - 1);
  const cardW = $derived(Math.min(340, vw - EDGE * 2));

  function routeMatches(route: string): boolean {
    const u = new URL(route, "http://x");
    if (u.pathname !== page.url.pathname) return false;
    for (const [k, v] of u.searchParams) if (page.url.searchParams.get(k) !== v) return false;
    return true;
  }

  function visible(node: HTMLElement): boolean {
    const r = node.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  }

  function find(target: string): HTMLElement | null {
    const all = document.querySelectorAll<HTMLElement>(`[data-tour="${target}"]`);
    for (const n of all) if (visible(n)) return n;
    return null;
  }

  /** Tunggu elemen muncul (halaman baru dimuat / data belum datang). */
  async function waitFor(target: string, ms: number, my: number): Promise<HTMLElement | null> {
    const until = performance.now() + ms;
    while (my === token) {
      const n = find(target);
      if (n || performance.now() > until) return n;
      await new Promise((r) => setTimeout(r, 100));
    }
    return null;
  }

  async function show(i: number) {
    const my = ++token;
    ready = false;
    el = null;
    box = null;
    const s = TOUR_STEPS[i];
    if (s.route && !routeMatches(s.route)) await goto(s.route).catch(() => {});
    const found = await waitFor(s.target, s.optional ? 1500 : 4000, my);
    if (my !== token) return;
    if (!found && s.optional) {
      move(dir);
      return;
    }
    el = found;
    found?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    ready = true;
  }

  function move(d: number) {
    dir = d;
    const n = tour.index + d;
    if (n >= total) endTour();
    else tour.index = Math.max(0, n);
  }

  // Ganti langkah → cari elemen langkah itu.
  $effect(() => {
    if (tour.active) {
      const i = tour.index;
      // URL yang dibaca di dalam show() tidak boleh jadi dependensi (halaman Meeting mengganti ?m=).
      untrack(() => show(i));
    } else {
      token++;
      el = null;
      box = null;
    }
  });

  // Ikuti posisi elemen tiap frame (gulir, ubah ukuran, isi halaman berubah).
  $effect(() => {
    if (!tour.active) return;
    let raf = 0;
    const loop = () => {
      vw = window.innerWidth;
      vh = window.innerHeight;
      let node = el;
      if (node && !node.isConnected) node = el = find(step.target);
      if (node) {
        const r = node.getBoundingClientRect();
        const b = { top: r.top - PAD, left: r.left - PAD, width: r.width + PAD * 2, height: r.height + PAD * 2 };
        if (!box || box.top !== b.top || box.left !== b.left || box.width !== b.width || box.height !== b.height) box = b;
      } else if (box) {
        box = null;
      }
      raf = requestAnimationFrame(loop);
    };
    loop();
    return () => cancelAnimationFrame(raf);
  });

  // Langkah "klik untuk lanjut": klik elemen asli memajukan tur.
  $effect(() => {
    const node = el;
    if (!node || !step.advanceOnClick) return;
    const onClick = () => setTimeout(() => move(1), 0);
    node.addEventListener("click", onClick);
    return () => node.removeEventListener("click", onClick);
  });

  /** Posisi kartu + panah: kanan → bawah → atas → kiri, sesuai ruang yang tersedia. */
  const place = $derived.by(() => {
    const w = cardW;
    const h = cardH;
    if (!box) return { side: "none" as const, x: (vw - w) / 2, y: Math.max(EDGE, (vh - h) / 2), arrow: 0 };
    const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
    const cx = box.left + box.width / 2;
    const cy = box.top + box.height / 2;
    const right = vw - (box.left + box.width);
    const below = vh - (box.top + box.height);
    if (right >= w + GAP + EDGE && box.width < vw / 3) {
      const y = clamp(cy - h / 2, EDGE, vh - h - EDGE);
      return { side: "right" as const, x: box.left + box.width + GAP, y, arrow: clamp(cy - y, 18, h - 18) };
    }
    if (below >= h + GAP + EDGE) {
      const x = clamp(cx - w / 2, EDGE, vw - w - EDGE);
      return { side: "bottom" as const, x, y: box.top + box.height + GAP, arrow: clamp(cx - x, 18, w - 18) };
    }
    if (box.top >= h + GAP + EDGE) {
      const x = clamp(cx - w / 2, EDGE, vw - w - EDGE);
      return { side: "top" as const, x, y: box.top - GAP - h, arrow: clamp(cx - x, 18, w - 18) };
    }
    if (box.left >= w + GAP + EDGE) {
      const y = clamp(cy - h / 2, EDGE, vh - h - EDGE);
      return { side: "left" as const, x: box.left - GAP - w, y, arrow: clamp(cy - y, 18, h - 18) };
    }
    // Elemen sebesar layar: kartu di bawah tengah, menutupi sebagian elemen.
    return { side: "none" as const, x: (vw - w) / 2, y: vh - h - EDGE * 2, arrow: 0 };
  });

  function typing(e: KeyboardEvent): boolean {
    const n = e.target as HTMLElement | null;
    return !!n && (n.tagName === "INPUT" || n.tagName === "TEXTAREA" || n.isContentEditable);
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (!tour.active) return;
    if (e.key === "Escape") endTour();
    else if (typing(e)) return;
    else if (e.key === "ArrowRight") move(1);
    else if (e.key === "ArrowLeft" && tour.index > 0) move(-1);
  }}
/>

{#if tour.active}
  <div class="pointer-events-none fixed inset-0 z-[60] print:hidden">
    <!-- Empat panel gelap di sekeliling sorotan: elemen yang disorot tetap bisa diklik dan diketik. -->
    {#if box}
      <div class="tour-dim" style:top="0" style:left="0" style:right="0" style:height="{Math.max(0, box.top)}px"></div>
      <div class="tour-dim" style:top="{box.top + box.height}px" style:left="0" style:right="0" style:bottom="0"></div>
      <div class="tour-dim" style:top="{box.top}px" style:left="0" style:width="{Math.max(0, box.left)}px" style:height="{box.height}px"></div>
      <div
        class="tour-dim"
        style:top="{box.top}px"
        style:left="{box.left + box.width}px"
        style:right="0"
        style:height="{box.height}px"
      ></div>
      <div
        class="tour-ring absolute rounded-[10px]"
        style:top="{box.top}px"
        style:left="{box.left}px"
        style:width="{box.width}px"
        style:height="{box.height}px"
      ></div>
    {:else}
      <div class="tour-dim inset-0"></div>
    {/if}

    {#if ready}
      <div
        role="dialog"
        aria-modal="false"
        aria-labelledby="tour-title"
        aria-live="polite"
        class="pointer-events-auto absolute flex flex-col gap-3 rounded-[var(--radius-box)] border border-line bg-sheet p-5 text-ink shadow-[0_24px_60px_-20px_rgb(28_31_38/0.45)] transition-[top,left] duration-200"
        style:width="{cardW}px"
        style:left="{place.x}px"
        style:top="{place.y}px"
        bind:clientHeight={cardH}
      >
        {#if place.side !== "none"}
          <span
            class={["tour-arrow", `tour-arrow-${place.side}`]}
            style:top={place.side === "left" || place.side === "right" ? `${place.arrow}px` : undefined}
            style:left={place.side === "top" || place.side === "bottom" ? `${place.arrow}px` : undefined}
            aria-hidden="true"
          ></span>
        {/if}

        <div class="flex items-start justify-between gap-3">
          <h2 id="tour-title" class="section-title leading-snug">{text.title}</h2>
          <button type="button" class="btn btn-quiet btn-icon btn-sm -m-1.5 shrink-0" aria-label={t.skip} title={t.skip} onclick={endTour}>
            <Icon name="x" size={16} />
          </button>
        </div>
        <p class="text-sm leading-relaxed text-ink-soft">{text.text}</p>
        {#if text.tryIt}
          <p class="flex gap-2 rounded-lg bg-paper px-3 py-2 text-sm leading-relaxed">
            <span class="shrink-0 font-semibold">{t.tryIt}</span>
            <span>{text.tryIt}</span>
          </p>
        {/if}

        <!-- Posisi langkah tepat di atas tombol navigasi. -->
        <div class="mt-1 flex items-center gap-2.5 border-t border-line-soft pt-3">
          <span class="tabular text-sm font-semibold text-ink-faint">{t.stepOf(tour.index + 1, total)}</span>
          <div class="flex gap-1" aria-hidden="true">
            {#each TOUR_STEPS as s, i (s.key)}
              <span class={["h-1.5 rounded-full transition-all", i === tour.index ? "w-4 bg-ink" : "w-1.5 bg-line"]}></span>
            {/each}
          </div>
        </div>
        <div class="flex items-center justify-between gap-2">
          {#if !last}
            <button type="button" class="text-sm text-ink-soft underline-offset-2 hover:text-ink hover:underline" onclick={endTour}>
              {t.skip}
            </button>
          {:else}
            <span></span>
          {/if}
          <div class="flex items-center gap-1.5">
            {#if tour.index > 0}
              <button type="button" class="btn btn-quiet btn-sm" onclick={() => move(-1)}>{t.prev}</button>
            {/if}
            <button type="button" class="btn btn-ink btn-sm" onclick={() => (last ? endTour() : move(1))}>
              {last ? t.done : t.next}
              {#if !last}<Icon name="arrow-right" size={14} />{/if}
            </button>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .tour-dim {
    position: absolute;
    pointer-events: auto;
    background: rgb(30 36 51 / 0.5);
  }
  .tour-ring {
    box-shadow:
      0 0 0 2px var(--color-sheet),
      0 0 0 4px var(--color-ink-strong);
  }
  @media (prefers-reduced-motion: no-preference) {
    .tour-ring {
      animation: tour-pulse 1.6s ease-in-out infinite;
    }
  }
  @keyframes tour-pulse {
    0%,
    100% {
      box-shadow:
        0 0 0 2px var(--color-sheet),
        0 0 0 4px var(--color-ink-strong);
    }
    50% {
      box-shadow:
        0 0 0 2px var(--color-sheet),
        0 0 0 4px var(--color-ink-strong),
        0 0 0 10px rgb(255 255 255 / 0.25);
    }
  }
  /* Panah kartu menunjuk ke elemen yang disorot. */
  .tour-arrow {
    position: absolute;
    width: 14px;
    height: 14px;
    background: var(--color-sheet);
    border: 1px solid var(--color-line);
    transform: translate(-50%, -50%) rotate(45deg);
  }
  .tour-arrow-right {
    left: 0;
    border-top-color: transparent;
    border-right-color: transparent;
  }
  .tour-arrow-left {
    left: 100%;
    border-bottom-color: transparent;
    border-left-color: transparent;
  }
  .tour-arrow-bottom {
    top: 0;
    border-right-color: transparent;
    border-bottom-color: transparent;
  }
  .tour-arrow-top {
    top: 100%;
    border-top-color: transparent;
    border-left-color: transparent;
  }
</style>
