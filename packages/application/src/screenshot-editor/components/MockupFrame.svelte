<script lang="ts" module>
  import type { Mockup } from "../types";

  export interface MockupFrameProps {
    mockup: Mockup;
    /** Corner radius (px) applied to the whole window. */
    radius: number;
    /** Ready-to-use CSS `box-shadow` value, or "none". */
    shadow: string;
    /** Ready-to-use CSS `border` value, or "none". */
    border: string;
    src: string;
    alt: string;
  }
</script>

<script lang="ts">
  import { ArrowLeft, ArrowRight, Lock, Plus, RotateCw } from "@lucide/svelte";

  let { mockup, radius, shadow, border, src, alt }: MockupFrameProps = $props();

  const isBrowser = $derived(mockup.kind === "safari" || mockup.kind === "chrome");
  const isDevice = $derived(mockup.kind === "phone" || mockup.kind === "tablet");
</script>

{#if isDevice}
  <!-- Device frame: a fixed-aspect bezel, screenshot cover-filled like a real
       screen. Uses its own radii; the drop shadow still applies. -->
  <div class="device" class:phone={mockup.kind === "phone"} class:tablet={mockup.kind === "tablet"} style:box-shadow={shadow}>
    <img class="device-screen" {src} {alt} />
    {#if mockup.kind === "phone"}
      <span class="notch" aria-hidden="true"></span>
    {/if}
  </div>
{:else}

<!-- The frame fills the padded stage; the image contains inside the content
     area so any aspect mismatch reads as a browser showing a page, not a
     stretch. Explicit colors: this is an illustration of window chrome, fixed
     regardless of the app theme (toggled only by the mockup's own light/dark). -->
<div
  class="mockup"
  class:dark={mockup.theme === "dark"}
  style:border-radius={`${radius}px`}
  style:box-shadow={shadow}
  style:border={border}
>
  {#if mockup.kind === "chrome"}
    <div class="bar tabstrip">
      <span class="lights"><i class="r"></i><i class="y"></i><i class="g"></i></span>
      <span class="tab">{mockup.url}</span>
      <Plus class="glyph" />
    </div>
    <div class="bar toolbar">
      <span class="navbtns">
        <ArrowLeft class="glyph" />
        <ArrowRight class="glyph" />
        <RotateCw class="glyph" />
      </span>
      <span class="address chrome">{mockup.url}</span>
    </div>
  {:else}
    <div class="bar single">
      <span class="lights"><i class="r"></i><i class="y"></i><i class="g"></i></span>
      {#if mockup.kind === "safari"}
        <span class="address safari"><Lock class="glyph sm" />{mockup.url}</span>
      {/if}
    </div>
  {/if}

  <div class="content">
    <img class="shot" {src} {alt} />
  </div>
</div>
{/if}

<style>
  .mockup {
    /* Fill the padded stage; backdrop shows only in the padding band. */
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: #ffffff;
    /* Light-theme palette (overridden by .dark below). */
    --bar: #f2f2f4;
    --bar-2: #e9e9ec;
    --edge: #dcdce1;
    --text: #3c3c43;
    --muted: #8a8a8f;
    --field: #ffffff;
    --field-edge: #e2e2e6;
  }

  .mockup.dark {
    background: #1c1c1e;
    --bar: #2c2c2e;
    --bar-2: #262628;
    --edge: #000000;
    --text: #e6e6ea;
    --muted: #98989d;
    --field: #1c1c1e;
    --field-edge: #3a3a3c;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex: none;
    padding: 0 0.75rem;
    background: var(--bar);
    border-bottom: 1px solid var(--edge);
    color: var(--muted);
  }

  .single {
    height: 2.25rem;
  }
  .tabstrip {
    height: 2.1rem;
    background: var(--bar-2);
  }
  .toolbar {
    height: 2.5rem;
  }

  .lights {
    display: inline-flex;
    gap: 0.5rem;
    flex: none;
  }
  .lights i {
    width: 0.72rem;
    height: 0.72rem;
    border-radius: 9999px;
    display: block;
  }
  .lights .r {
    background: #ff5f57;
  }
  .lights .y {
    background: #febc2e;
  }
  .lights .g {
    background: #28c840;
  }

  .tab {
    display: inline-flex;
    align-items: center;
    max-width: 60%;
    padding: 0.3rem 0.75rem;
    border-radius: 0.5rem 0.5rem 0 0;
    background: var(--field);
    color: var(--text);
    font-size: 0.8rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .navbtns {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    flex: none;
  }

  .address {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    height: 1.5rem;
    padding: 0 0.75rem;
    border-radius: 9999px;
    background: var(--field);
    border: 1px solid var(--field-edge);
    color: var(--text);
    font-size: 0.8rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .address.safari {
    margin: 0 auto;
    min-width: 45%;
    justify-content: center;
  }
  .address.chrome {
    flex: 1;
  }

  :global(.mockup .glyph) {
    width: 1rem;
    height: 1rem;
    stroke-width: 2;
  }
  :global(.mockup .glyph.sm) {
    width: 0.8rem;
    height: 0.8rem;
  }

  .content {
    flex: 1;
    min-height: 0;
    background: var(--field);
  }
  .shot {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  /* Device frames: fixed aspect, dark bezel, cover-filled screen. */
  .device {
    position: relative;
    max-width: 100%;
    max-height: 100%;
    background: #0b0b0d;
    overflow: hidden;
  }
  .device.phone {
    aspect-ratio: 9 / 19.5;
    height: 100%;
    border-radius: clamp(1.5rem, 8%, 3rem);
    padding: clamp(0.35rem, 1.6%, 0.7rem);
  }
  .device.tablet {
    aspect-ratio: 3 / 4;
    height: 100%;
    border-radius: clamp(0.75rem, 3%, 1.5rem);
    padding: clamp(0.5rem, 2%, 1rem);
  }
  .device-screen {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: inherit;
  }
  .notch {
    position: absolute;
    top: clamp(0.6rem, 2.4%, 1.1rem);
    left: 50%;
    transform: translateX(-50%);
    width: 34%;
    height: clamp(0.5rem, 2.2%, 1rem);
    background: #0b0b0d;
    border-radius: 9999px;
    z-index: 1;
  }
</style>
