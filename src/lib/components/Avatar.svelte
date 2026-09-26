<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import {
    CONDITIONS, MOODS, SIGNALS, mix, rgba,
    type Condition, type Emotion, type Mood, type Signal, type SignalKind
  } from '$lib/avatar';
  import AvatarKey from './AvatarKey.svelte';

  /*
   * OMNIX avatar: a holographic HUD core.
   *
   * Colour has three layers (see $lib/avatar): mood tints the core and inner
   * rings, condition tints the outer halo, and signals fire ripples.
   *
   * Motion model: each mood has a motion profile (spin, sweep, pulse, wave,
   * glow). Every frame the live parameters chase the profile with
   * frame-rate-independent exponential damping, so changing mood never snaps.
   * Rendering is SVG transforms and one path per frame; no layout work.
   *
   * Extras: a boot sequence, decoding status text, a cursor target-lock
   * reticle, orbiting satellites for live tool calls, flick-to-spin rings
   * with inertia, and neural links between data motes while thinking.
   *
   * Activity layers (weights live in the profile, so they fade in and out
   * with the mood): 3D gyroscope rings, a radial equalizer, lightning arcs,
   * radar contacts, inward-spiralling particles, and outward sparks for
   * streamed tokens and celebrations. The whole core floats and blinks.
   *
   * The stage has no frame: there is no background box and the SVG is not
   * clipped, so glows and ripples fade into whatever the avatar sits on.
   */

  let {
    emotion = 'idle',
    isSpeaking = false,
    isWorking = false,
    micLevel = 0,
    condition = 'nominal',
    signal = null,
    cpu = null,
    memory = null,
    scale = 1,
    streamTick = 0
  }: {
    emotion?: Emotion;
    isSpeaking?: boolean;
    isWorking?: boolean;
    micLevel?: number;
    condition?: Condition;
    signal?: Signal | null;
    cpu?: number | null;
    memory?: number | null;
    /** Display size relative to the 400×300 design. */
    scale?: number;
    /** Bump once per streamed reply chunk; each bump sprays sparks toward the chat. */
    streamTick?: number;
  } = $props();

  // Stage geometry (SVG user units = px).
  const W = 400;
  const H = 300;
  const CX = 200;
  const CY = 146;
  const R = { halo: 122, a: 106, a2: 113, b: 92, wave: 74, d: 56, core: 24 };
  const WAVE_POINTS = 120;
  const DORMANT_AFTER_S = 75;
  const MAX_RIPPLES = 14;
  const BOOT_S = 2.4;
  const DECODE_S = 0.55;
  const DECODE_GLYPHS = '█▓▒░<>/\\#01ΣΔΛΞ';
  const LOCK_AFTER_S = 0.6;
  const MAX_SATELLITES = 4;
  const SAT_ORBIT = { rx: 150, ry: 104 };
  const SAT_TIMEOUT_S = 30;
  const LINK_RANGE = 80;
  const MAX_SPARKS = 90;
  const MAX_BLIPS = 10;
  const EQ_BARS = 40;
  const GYRO_R = 66;
  /** Below this much rotation (deg) a press on the core counts as a click, not a spin. */
  const DRAG_THRESHOLD = 6;
  const uid = Math.random().toString(36).slice(2, 8);

  type Profile = {
    spin: number; // ring speed multiplier
    sweep: number; // radar sweep, deg/s
    pulse: number; // core pulse, Hz
    wave: number; // waveform ring amplitude, px
    glow: number; // overall intensity 0..1
    wobble: number; // back-and-forth ring confusion 0..1
    motes: number; // orbiting data points
    gyro: number; // 3D gyroscope rings 0..1
    eq: number; // radial equalizer 0..1
    arcs: number; // lightning arcs 0..1
    sonar: number; // radar contacts 0..1
    inflow: number; // particles spiralling into the core 0..1
    bob: number; // float amplitude, px
  };
  const FX = { gyro: 0, eq: 0, arcs: 0, sonar: 0, inflow: 0, bob: 2.5 };
  const PROFILE: Record<Mood, Profile> = {
    idle: { spin: 1, sweep: 55, pulse: 0.3, wave: 1.6, glow: 0.72, wobble: 0, motes: 2, ...FX, bob: 3 },
    listening: { spin: 0.7, sweep: 80, pulse: 0.6, wave: 2, glow: 0.85, wobble: 0, motes: 2, ...FX, eq: 0.8, inflow: 0.6, bob: 1 },
    thinking: { spin: 2.6, sweep: 150, pulse: 0.9, wave: 3, glow: 0.88, wobble: 0, motes: 6, ...FX, gyro: 1, bob: 2 },
    processing: { spin: 3.4, sweep: 230, pulse: 1.3, wave: 3.5, glow: 0.9, wobble: 0, motes: 6, ...FX, gyro: 0.8, arcs: 0.25, bob: 1.5 },
    working: { spin: 2.4, sweep: 180, pulse: 1.5, wave: 2.4, glow: 0.92, wobble: 0, motes: 4, ...FX, arcs: 1, bob: 0.6 },
    speaking: { spin: 1.4, sweep: 90, pulse: 0.8, wave: 2.5, glow: 0.95, wobble: 0, motes: 3, ...FX, eq: 1, bob: 2 },
    focused: { spin: 0.55, sweep: 120, pulse: 0.5, wave: 0.8, glow: 0.82, wobble: 0, motes: 1, ...FX, gyro: 0.4, bob: 0.6 },
    happy: { spin: 1.6, sweep: 90, pulse: 0.9, wave: 3, glow: 1, wobble: 0, motes: 4, ...FX, bob: 7 },
    excited: { spin: 4.6, sweep: 280, pulse: 2, wave: 6, glow: 1, wobble: 0, motes: 8, ...FX, arcs: 0.3, bob: 5 },
    success: { spin: 2.2, sweep: 160, pulse: 1.2, wave: 4, glow: 1, wobble: 0, motes: 6, ...FX, bob: 3 },
    confused: { spin: 0.9, sweep: 45, pulse: 0.5, wave: 2.5, glow: 0.7, wobble: 1, motes: 2, ...FX, bob: 2 },
    error: { spin: 0.5, sweep: 30, pulse: 2.8, wave: 5, glow: 1, wobble: 0, motes: 0, ...FX, arcs: 0.5, bob: 0 },
    dormant: { spin: 0.12, sweep: 8, pulse: 0.12, wave: 0.4, glow: 0.28, wobble: 0, motes: 0, ...FX, bob: 1.5 },
    searching: { spin: 1.8, sweep: 260, pulse: 0.9, wave: 2, glow: 0.9, wobble: 0, motes: 3, ...FX, sonar: 1, gyro: 0.3, bob: 1.5 },
    remembering: { spin: 1.2, sweep: 70, pulse: 0.7, wave: 2, glow: 0.9, wobble: 0, motes: 4, ...FX, inflow: 1, gyro: 0.5, bob: 2 }
  };

  const STATUS: Record<Mood, string> = {
    idle: 'Standing by', thinking: 'Analyzing', speaking: 'Responding', working: 'Executing task',
    happy: 'Task complete', excited: 'Excellent', focused: 'Concentrating', confused: "Didn't catch that",
    success: 'Success', error: 'Something went wrong', listening: 'Listening', processing: 'Computing',
    dormant: 'Dormant · click to wake', searching: 'Searching', remembering: 'Committing to memory'
  };

  let dormant = $state(false);
  let showKey = $state(false);
  let mood = $derived<Mood>(
    emotion === 'idle' ? (isWorking ? 'working' : dormant ? 'dormant' : 'idle') : emotion
  );
  let swatch = $derived(MOODS[mood]);
  let halo = $derived(condition === 'nominal' ? null : CONDITIONS[condition]);

  // ---- Rendered state ----
  const v = $state({
    angA: 0, angB: 0, angD: 0, sweep: 0, core: 1, glow: 0.7,
    lookX: 0, lookY: 0, jx: 0, jy: 0, flicker: 1, bob: 0, blink: 1
  });
  let wavePath = $state('');
  let motes = $state.raw<{ x: number; y: number; r: number; o: number }[]>([]);
  type Ripple = { id: number; r: number; life: number; max: number; color: string; width: number };
  let ripples = $state.raw<Ripple[]>([]);
  let boot = $state(0);
  let shownStatus = $state('');
  const reticle = $state({ x: CX, y: CY, show: 0, lock: 0, rot: 0 });
  type Satellite = {
    ref: string; label: string; state: SignalKind; born: number; done: number;
    ang: number; x: number; y: number; o: number;
  };
  let satellites = $state.raw<Satellite[]>([]);
  let links = $state.raw<{ x1: number; y1: number; x2: number; y2: number; o: number }[]>([]);
  let gyro = $state.raw<{ rot: number; ry: number; o: number }[]>([]);
  let eqPath = $state('');
  let eqO = $state(0);
  let bolts = $state.raw<{ d: string; o: number }[]>([]);
  let boltO = $state(0);
  type Blip = { id: number; x: number; y: number; life: number };
  let blips = $state.raw<Blip[]>([]);
  /** Outward sparks move in x/y; `pull` sparks spiral inward in polar coordinates. */
  type Spark = {
    x: number; y: number; vx: number; vy: number; ang: number; rad: number;
    life: number; max: number; r: number; c: string; pull: boolean;
  };
  let sparks = $state.raw<Spark[]>([]);

  // ---- Simulation state ----
  const sim = {
    clock: 0, spin: 1, sweep: 55, pulse: 0.3, wave: 1.6, glow: 0.72, wobble: 0, motes: 2,
    kick: 0, phase: 0, aAcc: 0, bAcc: 0, dAcc: 0, orbit: 0,
    lastInteract: 0, pointerAt: -10, px: CX, py: CY,
    glitchUntil: 0, flickerUntil: 0, nextScan: 6, nextSpark: 0, rippleId: 0,
    echoAt: 0, echoColor: '',
    pointerIn: false, locked: false, decodeTarget: '', decodeStart: 0,
    spinOffset: 0, spinVel: 0, dragged: false,
    gyro: 0, eq: 0, arcs: 0, sonar: 0, inflow: 0, bob: 3,
    nextBolt: 0, nextBlink: 4, blinkAt: -1, flash: 0, lastStream: -1, inCarry: 0, blipCarry: 0, blipId: 0,
    drag: null as null | { last: number; at: number; total: number }
  };
  let reducedMotion = false;
  let stageEl: HTMLDivElement | undefined = $state();

  // ---- Helpers ----
  const damp = (cur: number, target: number, lambda: number, dt: number) =>
    cur + (target - cur) * (1 - Math.exp(-lambda * dt));
  const rand = (a: number, b: number) => a + Math.random() * (b - a);
  const clamp = (x: number, a: number, b: number) => Math.min(b, Math.max(a, x));
  /** stroke-dasharray for a circle of radius `r`, from segment lengths in degrees. */
  const dash = (r: number, ...deg: number[]) => deg.map((d) => ((d * Math.PI * r) / 180).toFixed(2)).join(' ');
  const easeOutBack = (x: number) => 1 + 2.2 * (x - 1) ** 3 + 1.2 * (x - 1) ** 2;
  /** Boot reveal for a layer that powers on at `at` (0..1 of the boot sequence). */
  const reveal = (at: number) => clamp((boot - at) / 0.16, 0, 1);

  /** Scramble `text` so that the first `p` fraction of characters has resolved. */
  function decode(text: string, p: number): string {
    if (p >= 1) return text;
    let out = '';
    for (let i = 0; i < text.length; i++) {
      out += text[i] === ' ' || i / text.length < p ? text[i] : DECODE_GLYPHS[Math.floor(Math.random() * DECODE_GLYPHS.length)];
    }
    return out;
  }

  function ripple(color: string, strength = 1) {
    ripples = [
      ...ripples,
      { id: ++sim.rippleId, r: R.core + 6, life: 1.2, max: 1.2, color, width: 2.4 * strength }
    ].slice(-MAX_RIPPLES);
  }

  function spark(s: Partial<Spark> & { x: number; y: number }) {
    const next: Spark = { vx: 0, vy: 0, ang: 0, rad: 0, life: 1, max: 1, r: 1.6, c: swatch.color, pull: false, ...s };
    next.max = next.life;
    sparks = [...sparks.slice(-(MAX_SPARKS - 1)), next];
  }

  /** `n` sparks flying out of the core within `spread` degrees of `dir` (0 = right, 90 = down). */
  function burst(n: number, colors: string[], speed: number, dir = 0, spread = 360, life = 1.1) {
    if (reducedMotion) return;
    for (let i = 0; i < n; i++) {
      const a = ((dir + rand(-spread / 2, spread / 2)) * Math.PI) / 180;
      const sp = speed * rand(0.55, 1.15);
      spark({
        x: CX + Math.cos(a) * (R.core + 4), y: CY + Math.sin(a) * (R.core + 4),
        vx: Math.cos(a) * sp, vy: Math.sin(a) * sp,
        life: life * rand(0.7, 1.2), r: rand(1, 2.4), c: colors[i % colors.length]
      });
    }
  }

  /** A jagged bolt from the core shell out to ring A. */
  function bolt(): string {
    const a = rand(0, Math.PI * 2);
    const steps = 7;
    let d = '';
    for (let i = 0; i <= steps; i++) {
      const r = R.core + 6 + ((R.a - R.core - 6) * i) / steps;
      const j = i && i < steps ? rand(-0.16, 0.16) : 0;
      d += `${i ? 'L' : 'M'}${(CX + r * Math.cos(a + j)).toFixed(1)} ${(CY + r * Math.sin(a + j)).toFixed(1)}`;
    }
    return d;
  }

  function frame(dt: number) {
    sim.clock += dt;
    const t = sim.clock;
    const p = PROFILE[mood];
    const still = reducedMotion;

    // 0. Boot sequence and decoding status text.
    if (boot < 1) boot = still ? 1 : clamp(t / BOOT_S, 0, 1);
    if (statusText !== sim.decodeTarget) {
      sim.decodeTarget = statusText;
      sim.decodeStart = t;
    }
    const decodeP = still ? 1 : (t - sim.decodeStart) / DECODE_S;
    if (decodeP < 1.2 || shownStatus !== sim.decodeTarget) shownStatus = decode(sim.decodeTarget, decodeP);

    // 1. Dormancy after a quiet spell.
    if (emotion !== 'idle' || isSpeaking || isWorking) sim.lastInteract = t;
    const sleepy = t - sim.lastInteract > DORMANT_AFTER_S;
    if (sleepy !== dormant) {
      dormant = sleepy;
      if (!sleepy) {
        sim.kick = 5; // waking spin-up
        ripple(MOODS.idle.color, 1.2);
      }
    }

    // 2. Chase the mood's motion profile.
    sim.spin = damp(sim.spin, p.spin, 2.5, dt);
    sim.sweep = damp(sim.sweep, p.sweep, 2.5, dt);
    sim.pulse = damp(sim.pulse, p.pulse, 3, dt);
    sim.wave = damp(sim.wave, p.wave, 3, dt);
    sim.wobble = damp(sim.wobble, p.wobble, 3, dt);
    sim.motes = damp(sim.motes, p.motes, 2, dt);
    sim.kick = damp(sim.kick, 0, 1.8, dt);
    sim.gyro = damp(sim.gyro, p.gyro, 2.5, dt);
    sim.eq = damp(sim.eq, p.eq, 4, dt);
    sim.arcs = damp(sim.arcs, p.arcs, 4, dt);
    sim.sonar = damp(sim.sonar, p.sonar, 3, dt);
    sim.inflow = damp(sim.inflow, p.inflow, 3, dt);
    sim.bob = damp(sim.bob, p.bob, 2, dt);
    sim.flash = damp(sim.flash, 0, 6, dt);

    // Voice energy: live mic level while listening, synthetic lip-flap while speaking.
    const talk = (Math.abs(Math.sin(t * 17)) + Math.abs(Math.sin(t * 9.7 + 1)) + Math.abs(Math.sin(t * 23.3 + 2))) / 3;
    const energy = mood === 'listening' ? clamp(micLevel, 0, 1) : isSpeaking ? 0.35 + 0.5 * talk : 0;

    // 3. Rings. Working steps ring A like a gear; confusion wobbles everything.
    const spin = still ? 0 : sim.spin + sim.kick;
    const wob = still ? 0 : sim.wobble * Math.sin(t * 2.4) * 32;
    sim.aAcc += 14 * spin * dt;
    sim.bAcc -= 9 * spin * dt;
    sim.dAcc += 40 * spin * dt;
    sim.orbit += (still ? 0 : 20 + 26 * spin) * dt;
    // Flicked rings coast on with inertia after a drag.
    if (!sim.drag) {
      sim.spinOffset += sim.spinVel * dt;
      sim.spinVel = damp(sim.spinVel, 0, 1.1, dt);
    }
    const aTarget = mood === 'working' ? Math.floor(sim.aAcc / 30) * 30 : sim.aAcc;
    v.angA = damp(v.angA, aTarget + wob + sim.spinOffset, 16, dt);
    v.angB = sim.bAcc - wob * 0.6 + sim.spinOffset * 0.6;
    v.angD = sim.dAcc + wob * 1.3 + sim.spinOffset * 1.4;
    v.sweep += (still ? 0 : sim.sweep + sim.kick * 40) * dt;

    // 4. Core pulse and intensity.
    sim.phase += 2 * Math.PI * sim.pulse * dt;
    const beat = mood === 'error' ? Math.max(0, Math.sin(sim.phase)) ** 4 : Math.sin(sim.phase);
    v.core = 1 + (still ? 0 : 0.06 * beat) + energy * 0.18 + sim.flash * 0.25;
    const target = p.glow * (condition === 'offline' ? 0.45 : 1) + energy * 0.25;
    sim.glow = damp(sim.glow, target, 3.5, dt);
    v.glow = clamp(sim.glow, 0, 1.2);

    // 5. Glitch (error) and stutter (offline).
    if (!still && mood === 'error' && Math.random() < 1.6 * dt) sim.glitchUntil = t + rand(0.06, 0.2);
    if (condition === 'offline' && Math.random() < 1.2 * dt) sim.flickerUntil = t + rand(0.05, 0.25);
    const glitching = !still && t < sim.glitchUntil;
    v.jx = glitching ? rand(-5, 5) : damp(v.jx, 0, 25, dt);
    v.jy = glitching ? rand(-2, 2) : damp(v.jy, 0, 25, dt);
    v.flicker = glitching || t < sim.flickerUntil ? rand(0.35, 0.85) : damp(v.flicker, 1, 20, dt);

    // 6. Core tracks the pointer, otherwise drifts.
    let lx = still ? 0 : Math.sin(t * 0.37) * 1.5;
    let ly = still ? 0 : Math.cos(t * 0.29) * 1;
    if (t - sim.pointerAt < 3 && mood !== 'dormant') {
      lx = clamp((sim.px - CX) / 28, -7, 7);
      ly = clamp((sim.py - CY) / 28, -5, 5);
    }
    v.lookX = damp(v.lookX, lx, 5, dt);
    v.lookY = damp(v.lookY, ly, 5, dt);

    // 6b. Float (a bounce when happy) and an occasional blink of the core.
    v.bob = still ? 0 : mood === 'happy' ? -Math.abs(Math.sin(t * 3.4)) * sim.bob : Math.sin(t * 0.9) * sim.bob;
    const blinks = mood === 'idle' || mood === 'focused' || mood === 'happy' || mood === 'speaking';
    if (!still && blinks && t > sim.nextBlink) {
      sim.blinkAt = t;
      sim.nextBlink = t + rand(3.5, 8);
    }
    const bt = (t - sim.blinkAt) / 0.2;
    v.blink = bt >= 0 && bt < 1 ? 1 - 0.85 * Math.sin(Math.PI * bt) : 1;

    // 7. Waveform ring.
    const amp = still ? 1 : sim.wave + energy * (mood === 'listening' ? 26 : 14);
    const jag = isSpeaking || mood === 'listening' ? energy : 0;
    let d = '';
    for (let i = 0; i <= WAVE_POINTS; i++) {
      const th = (i / WAVE_POINTS) * Math.PI * 2;
      const n =
        0.5 * Math.sin(3 * th + t * 2.1) +
        0.3 * Math.sin(7 * th - t * 3.3) +
        0.2 * Math.sin(13 * th + t * 5.7) +
        jag * 0.6 * Math.sin(29 * th + t * 11);
      const r = R.wave + amp * n;
      d += `${i ? 'L' : 'M'}${(CX + r * Math.cos(th)).toFixed(1)} ${(CY + r * Math.sin(th)).toFixed(1)}`;
    }
    wavePath = d + 'Z';

    // 8. Orbiting data motes.
    const count = Math.round(sim.motes);
    const next: typeof motes = [];
    for (let i = 0; i < count; i++) {
      const a = ((sim.orbit * (1 + i * 0.13) + i * 47) * Math.PI) / 180;
      const r = 62 + (i % 3) * 16 + Math.sin(t * 1.3 + i) * 3;
      next.push({ x: CX + r * Math.cos(a), y: CY + r * Math.sin(a) * 0.96, r: i % 2 ? 1.6 : 2.2, o: clamp(sim.motes - i, 0, 1) });
    }
    motes = next;

    // 8b. Neural links between nearby motes while the mind is busy.
    if (count >= 4 && !still) {
      const L: typeof links = [];
      for (let i = 0; i < next.length; i++) {
        for (let j = i + 1; j < next.length; j++) {
          const a = next[i];
          const b = next[j];
          const dist = Math.hypot(a.x - b.x, a.y - b.y);
          if (dist < LINK_RANGE) {
            const o = Math.min(1, 1.4 * (1 - dist / LINK_RANGE)) * (0.55 + 0.45 * Math.sin(t * 7 + i * 3.1 + j)) * Math.min(a.o, b.o);
            L.push({ x1: a.x, y1: a.y, x2: b.x, y2: b.y, o });
          }
        }
      }
      links = L;
    } else if (links.length) {
      links = [];
    }

    // 8c. Target-lock reticle on the cursor when it rests somewhere on the stage.
    const range = Math.hypot(sim.px - CX, sim.py - CY);
    const aiming = sim.pointerIn && !sim.drag && t - sim.pointerAt < 4 && range > 46 && mood !== 'dormant' && boot >= 1;
    const locked = aiming && t - sim.pointerAt > LOCK_AFTER_S;
    if (locked && !sim.locked) sim.kick = Math.max(sim.kick, 1.5);
    sim.locked = locked;
    reticle.show = damp(reticle.show, aiming ? 1 : 0, 8, dt);
    reticle.lock = damp(reticle.lock, locked ? 1 : 0, 12, dt);
    reticle.x = damp(reticle.x, sim.px, 16, dt);
    reticle.y = damp(reticle.y, sim.py, 16, dt);
    reticle.rot = locked ? damp(reticle.rot, Math.round(reticle.rot / 90) * 90, 12, dt) : reticle.rot + (still ? 0 : 80 * dt);

    // 8d. Tool satellites orbit while their call runs, then report and fade.
    if (satellites.length) {
      satellites = satellites
        .map((q) => {
          const ang = q.ang + (still ? 0 : 16 * dt);
          const rad = (ang * Math.PI) / 180;
          const end = q.done ? q.done + 1.6 : q.born + SAT_TIMEOUT_S;
          const o = Math.min(clamp((t - q.born) / 0.3, 0, 1), clamp((end + 0.4 - t) / 0.4, 0, 1));
          return { ...q, ang, o, x: CX + SAT_ORBIT.rx * Math.cos(rad), y: CY + SAT_ORBIT.ry * Math.sin(rad) };
        })
        .filter((q) => t < (q.done ? q.done + 2 : q.born + SAT_TIMEOUT_S + 0.4));
    }

    // 8e. Gyroscope: three tilted rings turning in 3D while the mind is busy.
    if (sim.gyro > 0.02 && !still) {
      gyro = [0, 60, 120].map((tilt, i) => {
        const ph = t * (0.9 + i * 0.31) * (0.7 + sim.spin * 0.2) + i * 1.7;
        return {
          rot: tilt + t * 14 * (i % 2 ? -1 : 1),
          ry: GYRO_R * Math.abs(Math.cos(ph)) + 1.5,
          o: sim.gyro * (0.3 + 0.45 * Math.abs(Math.sin(ph)))
        };
      });
    } else if (gyro.length) {
      gyro = [];
    }

    // 8f. Radial equalizer between the core and the inner rings.
    eqO = sim.eq;
    if (sim.eq > 0.02) {
      let e = '';
      for (let i = 0; i < EQ_BARS; i++) {
        const th = (i / EQ_BARS) * Math.PI * 2;
        const n = still ? 0.3 : 0.5 + 0.5 * Math.sin(th * 5 + t * 9 + Math.sin(t * 3 + i));
        const len = 1.5 + sim.eq * (2 + (energy * 13 + 3) * n);
        const r0 = R.core + 9;
        const c = Math.cos(th);
        const sn = Math.sin(th);
        e += `M${(CX + r0 * c).toFixed(1)} ${(CY + r0 * sn).toFixed(1)}L${(CX + (r0 + len) * c).toFixed(1)} ${(CY + (r0 + len) * sn).toFixed(1)}`;
      }
      eqPath = e;
    }

    // 8g. Lightning arcs crackle from the core while executing.
    boltO = sim.arcs;
    if (sim.arcs > 0.05 && !still) {
      if (t > sim.nextBolt) {
        const n = sim.arcs > 0.6 && Math.random() < 0.5 ? 2 : 1;
        bolts = Array.from({ length: n }, () => ({ d: bolt(), o: 0.5 + 0.5 * Math.random() }));
        sim.nextBolt = t + rand(0.05, 0.16) / Math.max(0.3, sim.arcs);
      } else {
        bolts = bolts.map((b) => ({ ...b, o: b.o * Math.exp(-9 * dt) }));
      }
    } else if (bolts.length) {
      bolts = [];
    }

    // 8h. Radar contacts light up under the sweep while searching.
    if (sim.sonar > 0.1 && !still) {
      sim.blipCarry += 3.2 * sim.sonar * dt;
      if (sim.blipCarry >= 1) {
        sim.blipCarry -= 1;
        const a = ((v.sweep - 4) * Math.PI) / 180;
        const r = rand(40, R.halo - 6);
        blips = [...blips.slice(-(MAX_BLIPS - 1)), { id: ++sim.blipId, x: CX + r * Math.cos(a), y: CY + r * Math.sin(a), life: 1.6 }];
      }
    }
    if (blips.length) blips = blips.map((b) => ({ ...b, life: b.life - dt })).filter((b) => b.life > 0);

    // 8i. Particles spiral into the core (voice coming in, memories being stored).
    if (sim.inflow > 0.05 && !still) {
      const rate = 16 * sim.inflow * (mood === 'listening' ? 0.3 + clamp(micLevel, 0, 1) * 2.2 : 1);
      sim.inCarry += rate * dt;
      while (sim.inCarry >= 1) {
        sim.inCarry -= 1;
        spark({
          x: CX, y: CY, ang: rand(0, Math.PI * 2), rad: rand(118, 150), life: 3, r: rand(1, 2), pull: true,
          c: Math.random() < 0.5 ? swatch.color : moodHi
        });
      }
    }

    // 8j. Advance sparks. Inward ones speed up and curl as they near the core.
    if (sparks.length) {
      const next: Spark[] = [];
      for (const q of sparks) {
        const n = { ...q, life: q.life - dt };
        if (q.pull) {
          n.rad = q.rad - (30 + 90 * (1 - q.rad / 150)) * dt;
          n.ang = q.ang + (1.2 + 40 / Math.max(20, q.rad)) * dt;
          n.x = CX + n.rad * Math.cos(n.ang);
          n.y = CY + n.rad * Math.sin(n.ang);
          if (n.rad <= R.core) {
            if (mood === 'remembering') sim.flash = Math.min(1, sim.flash + 0.35);
            continue;
          }
        } else {
          n.x = q.x + q.vx * dt;
          n.y = q.y + q.vy * dt;
          n.vx = q.vx * Math.exp(-1.4 * dt);
          n.vy = q.vy * Math.exp(-1.4 * dt) + 18 * dt;
        }
        if (n.life > 0) next.push(n);
      }
      sparks = next;
    }

    // 9. Ambient events.
    if (mood === 'idle' && t > sim.nextScan && !still) {
      sim.kick = 2.2;
      ripple(swatch.color, 0.5);
      sim.nextScan = t + rand(9, 16);
    }
    if (mood === 'excited' && t > sim.nextSpark) {
      ripple(Math.random() < 0.5 ? swatch.color : MOODS.happy.color, 0.6);
      burst(4, [swatch.color, MOODS.happy.color, '#ffffff'], 140);
      sim.nextSpark = t + rand(0.25, 0.5);
    }
    if (sim.echoAt && t > sim.echoAt) {
      ripple(sim.echoColor, 1.4);
      sim.echoAt = 0;
    }

    // 10. Ripples expand and fade.
    if (ripples.length) {
      ripples = ripples
        .map((q) => ({ ...q, r: still ? R.a : q.r + (170 - q.r * 0.6) * dt, life: q.life - dt }))
        .filter((q) => q.life > 0);
    }
  }

  // Mood changes fire a one-shot reaction.
  function react(m: Mood) {
    if (m !== 'dormant') sim.lastInteract = sim.clock;
    const c = MOODS[m].color;
    switch (m) {
      case 'success':
        sim.kick = 5;
        ripple(c, 1.6);
        sim.echoAt = sim.clock + 0.22;
        sim.echoColor = c;
        burst(28, [c, MOODS.happy.color, '#ffffff', MOODS.idle.color], 170, 0, 360, 1.4);
        break;
      case 'excited':
        sim.kick = 7;
        ripple(c, 1.4);
        burst(20, [c, MOODS.happy.color, '#ffffff'], 190);
        break;
      case 'happy':
        sim.kick = 2;
        ripple(c, 1);
        burst(8, [c, '#ffffff'], 90, -90, 120);
        break;
      case 'error':
        sim.glitchUntil = sim.clock + 0.7;
        ripple(c, 1.5);
        burst(12, [c, '#ffffff'], 220, 0, 360, 0.5);
        break;
      case 'searching':
        blips = [];
        sim.kick = 3;
        ripple(c, 0.7);
        break;
      case 'remembering':
        sim.kick = 2;
        ripple(c, 0.8);
        break;
      case 'listening':
      case 'thinking':
      case 'processing':
      case 'working':
      case 'speaking':
        sim.kick = 3;
        ripple(c, 0.7);
        break;
    }
  }

  function onSignal(s: Signal) {
    ripple(SIGNALS[s.kind].color, 0.9);
    if (!s.ref) return;
    const known = satellites.some((q) => q.ref === s.ref);
    if (s.kind === 'tool' && !known) {
      const sat: Satellite = {
        ref: s.ref, label: (s.label ?? 'tool').slice(0, 18), state: 'tool', born: sim.clock, done: 0,
        // Golden-angle spacing keeps concurrent satellites (and their labels) apart.
        ang: satellites.length ? satellites[satellites.length - 1].ang + 137.5 : rand(0, 360),
        x: CX, y: CY, o: 0
      };
      satellites = [...satellites.slice(-(MAX_SATELLITES - 1)), sat];
    } else if (known && (s.kind === 'ok' || s.kind === 'fail')) {
      satellites = satellites.map((q) => (q.ref === s.ref ? { ...q, state: s.kind, done: sim.clock } : q));
    }
  }

  function poke() {
    // A drag that turned into a spin already had its effect.
    if (sim.dragged) {
      sim.dragged = false;
      return;
    }
    sim.lastInteract = sim.clock;
    if (dormant) return; // the next frame wakes it
    sim.kick = 6;
    ripple(swatch.color, 1.3);
  }

  /** Pointer position in stage units; also records it for eye tracking and the reticle. */
  function track(e: PointerEvent): boolean {
    if (!stageEl) return false;
    const r = stageEl.getBoundingClientRect();
    if (!r.width) return false;
    sim.px = (e.clientX - r.left) * (W / r.width);
    sim.py = (e.clientY - r.top) * (H / r.height);
    sim.pointerIn = sim.px >= 0 && sim.px <= W && sim.py >= 0 && sim.py <= H;
    sim.pointerAt = sim.clock;
    sim.lastInteract = sim.clock;
    return true;
  }
  const pointerAngle = () => (Math.atan2(sim.py - CY, sim.px - CX) * 180) / Math.PI;

  function onPointerMove(e: PointerEvent) {
    if (!track(e) || !sim.drag) return;
    // Spin the rings with the pointer; velocity is kept for the flick.
    let delta = pointerAngle() - sim.drag.last;
    if (delta > 180) delta -= 360;
    if (delta < -180) delta += 360;
    const now = performance.now();
    const dtp = Math.max(1 / 120, (now - sim.drag.at) / 1000);
    sim.spinOffset += delta;
    sim.spinVel = clamp(damp(sim.spinVel, delta / dtp, 20, dtp), -1500, 1500);
    sim.drag.total += Math.abs(delta);
    sim.drag.last += delta;
    sim.drag.at = now;
  }

  function onCorePointerDown(e: PointerEvent) {
    if (e.button !== 0 || reducedMotion || !track(e)) return;
    try {
      (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    } catch {
      // Pointer already released; a plain click still works.
    }
    sim.drag = { last: pointerAngle(), at: performance.now(), total: 0 };
    sim.spinVel = 0;
  }

  function onCorePointerUp() {
    const d = sim.drag;
    sim.drag = null;
    if (!d) return;
    if (d.total > DRAG_THRESHOLD) {
      sim.dragged = true;
      sim.kick = Math.max(sim.kick, Math.min(6, Math.abs(sim.spinVel) / 250));
      if (Math.abs(sim.spinVel) > 600) ripple(swatch.color, 1);
    } else {
      sim.spinVel = 0;
    }
  }

  onMount(() => {
    reducedMotion = globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
    let raf = 0;
    let last = performance.now();
    const loop = (now: number) => {
      // Clamp dt so a backgrounded window doesn't jump the animation.
      const dt = Math.min(0.05, Math.max(0, (now - last) / 1000));
      last = now;
      frame(dt);
      raf = requestAnimationFrame(loop);
    };
    sim.kick = 6; // boot spin-up
    ripple(MOODS.idle.color, 1.2);
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    const m = mood;
    untrack(() => react(m));
  });
  $effect(() => {
    const s = signal;
    if (s) untrack(() => onSignal(s));
  });
  // Streamed reply chunks: a short spray of sparks from the core down toward the chat.
  $effect(() => {
    const n = streamTick;
    if (!n) return;
    untrack(() => {
      sim.lastInteract = sim.clock;
      if (sim.clock - sim.lastStream < 0.035) return;
      sim.lastStream = sim.clock;
      burst(2, [moodHi, swatch.color], 150, 90, 70, 0.9);
    });
  });
  $effect(() => {
    const c = condition;
    if (c !== 'nominal') {
      untrack(() => {
        ripple(CONDITIONS[c].color, 1.2);
        sim.flickerUntil = sim.clock + 0.3;
      });
    }
  });

  // ---- Derived presentation ----
  let moodHi = $derived(mix(swatch.color, '#ffffff', 0.6));
  let haloColor = $derived(halo?.color ?? swatch.color);
  let statusText = $derived(boot < 0.85 ? 'Initializing' : STATUS[mood]);
  let bootScale = $derived(0.72 + 0.28 * easeOutBack(clamp(boot / 0.7, 0, 1)));
  const pct = (n: number | null) => (n == null ? '--' : `${Math.round(n)}%`.padStart(4, ' '));
  const bar = (n: number | null) => clamp((n ?? 0) / 100, 0, 1) * 44;
  const load = (n: number | null, hot: number) => (n != null && n > hot ? CONDITIONS.strain.color : 'var(--mood)');
</script>

<svelte:window onpointermove={onPointerMove} />

<div class="avatar-wrap">
  <div
    class="avatar-stage"
    class:dormant={mood === 'dormant'}
    bind:this={stageEl}
    style="width: {W * scale}px; height: {H * scale}px; --mood: {swatch.color}; --mood-hi: {moodHi}; --mood-glow: {rgba(swatch.color, 0.45)}; --halo: {haloColor};"
  >
    <svg viewBox="0 0 {W} {H}" width="100%" height="100%" aria-hidden="true">
      <defs>
        <radialGradient id="ambient-{uid}">
          <stop offset="0%" class="stop-mood" stop-opacity="0.5" />
          <stop offset="100%" class="stop-mood" stop-opacity="0" />
        </radialGradient>
        <radialGradient id="core-{uid}">
          <stop offset="0%" stop-color="#ffffff" />
          <stop offset="28%" class="stop-hi" />
          <stop offset="62%" class="stop-mood" stop-opacity="0.55" />
          <stop offset="100%" class="stop-mood" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="sweep-{uid}" gradientUnits="userSpaceOnUse" x1={CX + R.halo} y1={CY - 70} x2={CX + R.halo} y2={CY}>
          <stop offset="0%" class="stop-mood" stop-opacity="0" />
          <stop offset="100%" class="stop-mood" stop-opacity="0.5" />
        </linearGradient>
        <filter id="glow-{uid}" x="-30%" y="-30%" width="160%" height="160%">
          <feGaussianBlur stdDeviation="2.4" result="b" />
          <feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
        <filter id="bloom-{uid}" x="-80%" y="-80%" width="260%" height="260%">
          <feGaussianBlur stdDeviation="7" />
        </filter>
      </defs>

      <!-- Ambient glow: a soft radial falloff, no backdrop box. -->
      <circle cx={CX} cy={CY + v.bob} r="175" fill="url(#ambient-{uid})" opacity={v.glow * 0.6} />

      <g
        transform="translate({v.jx} {v.jy + v.bob}) translate({CX} {CY}) scale({bootScale}) translate({-CX} {-CY})"
        opacity={v.flicker}
      >
        <!-- Far layer: halo, radar sweep, tool satellites -->
        <g transform="translate({v.lookX * 0.25} {v.lookY * 0.25})">
          <g transform="rotate({v.sweep} {CX} {CY})" opacity={reveal(0.62)}>
            <path
              d="M{CX} {CY}L{CX + R.halo * Math.cos(-0.9)} {CY + R.halo * Math.sin(-0.9)}A{R.halo} {R.halo} 0 0 1 {CX + R.halo} {CY}Z"
              fill="url(#sweep-{uid})"
              opacity={v.glow}
            />
            <line x1={CX} y1={CY} x2={CX + R.halo} y2={CY} class="sweep-edge" opacity={v.glow} />
          </g>

          <g opacity={reveal(0.52)}>
          <circle
            cx={CX} cy={CY} r={R.halo}
            class="halo {condition}"
            stroke-dasharray={condition === 'network' ? dash(R.halo, 14, 9) : dash(R.halo, 0.6, 1.4)}
          />
          <circle cx={CX} cy={CY} r={R.halo + 5} class="halo-rim" class:alert={halo} />
          {#if condition === 'security'}
            <g transform="rotate({-v.angB * 0.5} {CX} {CY})">
              {#each [0, 90, 180, 270] as a}
                <path
                  class="chevron"
                  transform="rotate({a} {CX} {CY})"
                  d="M{CX - 7} {CY - R.halo - 14}L{CX} {CY - R.halo - 7}L{CX + 7} {CY - R.halo - 14}"
                />
              {/each}
            </g>
          {/if}
          </g>

          {#each satellites as q (q.ref)}
            {@const lx = clamp(q.x, 52, W - 52)}
            <!-- Label sits on the outer side of the node, unless that would hit the HUD text or status line. -->
            {@const ly = q.y < CY ? (q.y - 13 < 72 ? q.y + 19 : q.y - 13) : q.y + 19 > 266 ? q.y - 13 : q.y + 19}
            <g class="sat" class:pending={q.state === 'tool'} opacity={q.o} style="--sat: {SIGNALS[q.state].color};">
              <line x1={CX} y1={CY} x2={q.x} y2={q.y} class="sat-link" />
              <g transform="translate({q.x} {q.y})">
                <circle r={q.done ? 9 + (sim.clock - q.done) * 10 : 9} class="sat-ring" />
                <rect x="-4" y="-4" width="8" height="8" transform="rotate(45)" class="sat-node" />
              </g>
              <text x={lx} y={ly} text-anchor="middle" class="sat-label">
                {q.label}{q.state === 'ok' ? ' ✓' : q.state === 'fail' ? ' ✗' : ''}
              </text>
            </g>
          {/each}
        </g>

        <!-- Mid layer: rings, waveform, motes -->
        <g transform="translate({v.lookX * 0.55} {v.lookY * 0.55})" filter="url(#glow-{uid})" opacity={0.35 + v.glow * 0.65}>
          <g transform="rotate({v.angA} {CX} {CY})" opacity={reveal(0.42)}>
            <circle cx={CX} cy={CY} r={R.a} class="ring-a" stroke-dasharray={dash(R.a, 78, 42)} />
            <circle cx={CX} cy={CY} r={R.a2} class="ring-a2" stroke-dasharray={dash(R.a2, 30, 6, 4, 6, 2, 72)} />
          </g>
          <g transform="rotate({v.angB} {CX} {CY})" opacity={reveal(0.32)}>
            <circle cx={CX} cy={CY} r={R.b} class="ring-b" stroke-dasharray={dash(R.b, 0.7, 3.05)} />
            <circle cx={CX} cy={CY} r={R.b} class="ring-b-major" stroke-dasharray={dash(R.b, 1.6, 43.4)} />
          </g>
          <path d={wavePath} class="wave" opacity={reveal(0.22)} />
          {#each gyro as g, i}
            <ellipse cx={CX} cy={CY} rx={GYRO_R} ry={g.ry} transform="rotate({g.rot} {CX} {CY})" class="gyro" class:alt={i === 1} opacity={g.o} />
          {/each}
          {#if eqO > 0.02}
            <path d={eqPath} class="eq" opacity={eqO} />
          {/if}
          <g transform="rotate({v.angD} {CX} {CY})" opacity={reveal(0.12)}>
            <circle cx={CX} cy={CY} r={R.d} class="ring-d" stroke-dasharray={dash(R.d, 64, 26)} />
          </g>
          <g transform="rotate({-v.angD * 0.7} {CX} {CY})" opacity={reveal(0.12)}>
            <circle cx={CX} cy={CY} r={R.d - 7} class="ring-d2" stroke-dasharray={dash(R.d - 7, 20, 100)} />
          </g>
          {#each links as l}
            <line x1={l.x1} y1={l.y1} x2={l.x2} y2={l.y2} class="neural" opacity={l.o} />
          {/each}
          {#each motes as m}
            <circle cx={m.x} cy={m.y} r={m.r} class="mote" opacity={m.o} />
          {/each}
        </g>

        <!-- Radar contacts and lightning -->
        {#each blips as b (b.id)}
          <g class="blip" opacity={Math.min(1, b.life / 0.6)}>
            <circle cx={b.x} cy={b.y} r="2.2" />
            <circle cx={b.x} cy={b.y} r={3 + (1.6 - b.life) * 9} class="blip-ring" />
          </g>
        {/each}
        {#each bolts as b}
          <path d={b.d} class="bolt" opacity={b.o * boltO} filter="url(#glow-{uid})" />
        {/each}

        <!-- Near layer: the core. It blinks by squashing vertically. -->
        <g transform="translate({v.lookX} {v.lookY}) translate({CX} {CY}) scale(1 {v.blink}) translate({-CX} {-CY})" opacity={reveal(0)}>
          <circle cx={CX} cy={CY} r={R.core * 2.2 * v.core * (1 + (1 - reveal(0.1)) * 1.5)} fill="url(#core-{uid})" filter="url(#bloom-{uid})" opacity={v.glow} />
          <circle cx={CX} cy={CY} r={R.core * v.core} fill="url(#core-{uid})" opacity={0.4 + v.glow * 0.6} />
          <circle cx={CX} cy={CY} r={R.core + 6} class="core-shell" />
          <g transform="rotate({-v.angD * 1.4} {CX} {CY})" class="reactor">
            <polygon points="{CX},{CY - 15} {CX + 13},{CY + 7.5} {CX - 13},{CY + 7.5}" />
            <polygon points="{CX},{CY + 15} {CX + 13},{CY - 7.5} {CX - 13},{CY - 7.5}" opacity="0.45" />
          </g>
          <circle cx={CX} cy={CY} r={5 + v.core * 2} class="heart" />
        </g>

        {#each sparks as q}
          <circle cx={q.x} cy={q.y} r={q.r} style="fill: {q.c};" opacity={Math.min(1, (q.life / q.max) * 1.6)} />
        {/each}

        {#each ripples as q (q.id)}
          <circle
            cx={CX} cy={CY} r={q.r}
            class="ripple"
            style="stroke: {q.color};"
            stroke-width={q.width * (q.life / q.max)}
            opacity={Math.min(1, (q.life / q.max) * 1.4)}
          />
        {/each}
      </g>

      <!-- Target-lock reticle -->
      {#if reticle.show > 0.02}
        {@const gap = 14 - 6 * reticle.lock}
        {@const dx = Math.round(reticle.x - CX)}
        {@const dy = Math.round(CY - reticle.y)}
        <g class="reticle" opacity={reticle.show}>
          <line x1={CX + v.lookX} y1={CY + v.lookY} x2={reticle.x} y2={reticle.y} class="tether" opacity={0.25 + reticle.lock * 0.5} />
          <g transform="translate({reticle.x} {reticle.y}) rotate({reticle.rot})">
            {#each [0, 90, 180, 270] as a}
              <path transform="rotate({a})" d="M{gap - 5} {-gap}H{gap}V{-gap + 5}" />
            {/each}
            <circle r={1.5 + reticle.lock} class="reticle-dot" />
          </g>
          {#if reticle.lock > 0.5}
            {@const right = reticle.x < W - 90}
            <g class="reticle-label" opacity={(reticle.lock - 0.5) * 2}>
              <text x={reticle.x + (right ? 18 : -18)} y={reticle.y - 6} text-anchor={right ? 'start' : 'end'} class="lock">LOCK</text>
              <text x={reticle.x + (right ? 18 : -18)} y={reticle.y + 5} text-anchor={right ? 'start' : 'end'}>
                X{dx >= 0 ? '+' : ''}{dx} Y{dy >= 0 ? '+' : ''}{dy}
              </text>
              <text x={reticle.x + (right ? 18 : -18)} y={reticle.y + 15} text-anchor={right ? 'start' : 'end'}>
                RNG {String(Math.round(Math.hypot(dx, dy))).padStart(3, '0')}
              </text>
            </g>
          {/if}
        </g>
      {/if}

      <!-- HUD readouts -->
      <g class="hud" opacity={reveal(0.72)}>
        <text x="16" y="24" class="hud-title">OMNIX // CORE</text>
        <text x="16" y="38">MODE <tspan class="hud-val">{swatch.label.toUpperCase()}</tspan></text>
        <text x="16" y="50">LINK <tspan class="hud-val" style="fill: {condition === 'offline' ? CONDITIONS.offline.color : 'var(--mood)'};">{condition === 'offline' ? 'LOST' : 'ONLINE'}</tspan></text>
        {#if cpu != null || memory != null}
          <text x="384" y="24" text-anchor="end" class="hud-title">SYS LOAD</text>
          <text x="330" y="38" text-anchor="end">CPU</text>
          <rect x="336" y="32" width="44" height="4" class="gauge-bg" />
          <rect x="336" y="32" width={bar(cpu)} height="4" style="fill: {load(cpu, 85)};" />
          <text x="330" y="50" text-anchor="end">MEM</text>
          <rect x="336" y="44" width="44" height="4" class="gauge-bg" />
          <rect x="336" y="44" width={bar(memory)} height="4" style="fill: {load(memory, 90)};" />
          <text x="384" y="62" text-anchor="end" class="hud-dim">{pct(cpu)} / {pct(memory)}</text>
        {/if}
      </g>
    </svg>

    {#if halo}
      <div class="banner" transition:fade={{ duration: 200 }} role="status">⚠ {halo.label}</div>
    {/if}

    <button
      class="poke"
      style="left: {((CX - 42) / W) * 100}%; top: {((CY - 42) / H) * 100}%; width: {(84 / W) * 100}%; height: {(84 / H) * 100}%;"
      onclick={poke}
      onpointerdown={onCorePointerDown}
      onpointerup={onCorePointerUp}
      onpointercancel={onCorePointerUp}
      aria-label={mood === 'dormant' ? 'Wake OMNIX' : 'Ping OMNIX'}
      title={mood === 'dormant' ? 'Wake me' : 'Click to ping · drag to spin'}
    ></button>

    <button
      class="key-toggle"
      class:on={showKey}
      onclick={() => (showKey = !showKey)}
      aria-expanded={showKey}
      title="Colour key"
    >KEY</button>

    <div class="status-text" aria-live="polite" aria-label={statusText}>{shownStatus}</div>
  </div>

  {#if showKey}
    <div transition:fade={{ duration: 150 }}><AvatarKey {mood} {condition} /></div>
  {/if}
</div>

<style>
  /* Registered so colour changes cross-fade instead of snapping. */
  @property --mood { syntax: '<color>'; inherits: true; initial-value: #29d8ff; }
  @property --mood-hi { syntax: '<color>'; inherits: true; initial-value: #a9efff; }
  @property --mood-glow { syntax: '<color>'; inherits: true; initial-value: rgba(41, 216, 255, 0.45); }
  @property --halo { syntax: '<color>'; inherits: true; initial-value: #29d8ff; }

  .avatar-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  /* Frameless: no box, no clipping. Glows and ripples spill past the stage. */
  .avatar-stage {
    position: relative;
    transition:
      --mood 0.8s ease, --mood-hi 0.8s ease, --mood-glow 0.8s ease, --halo 0.6s ease,
      width 0.5s ease, height 0.5s ease;
    user-select: none;
  }
  svg { display: block; overflow: visible; pointer-events: none; }

  .stop-mood { stop-color: var(--mood); }
  .stop-hi { stop-color: var(--mood-hi); }

  .sweep-edge { stroke: var(--mood-hi); stroke-width: 1; stroke-opacity: 0.55; }

  .halo {
    fill: none;
    stroke: var(--halo);
    stroke-width: 5;
    opacity: 0.35;
  }
  .halo.offline { opacity: 0.8; animation: stutter 2.2s steps(1) infinite; }
  .halo.strain { opacity: 0.95; stroke-width: 7; animation: tick-crawl 0.6s linear infinite; }
  .halo.security { opacity: 1; stroke-width: 6; animation: alarm 0.9s ease-in-out infinite; }
  .halo.network { opacity: 0.9; stroke-width: 3; animation: dash-crawl 1.6s linear infinite; }
  .halo-rim { fill: none; stroke: var(--halo); stroke-width: 0.8; opacity: 0.25; }
  .halo-rim.alert { opacity: 0.8; animation: alarm 0.9s ease-in-out infinite; }
  .chevron { fill: none; stroke: var(--halo); stroke-width: 2.5; stroke-linecap: round; stroke-linejoin: round; animation: alarm 0.9s ease-in-out infinite; }

  .ring-a { fill: none; stroke: var(--mood); stroke-width: 3; stroke-linecap: round; }
  .ring-a2 { fill: none; stroke: var(--mood-hi); stroke-width: 1.2; opacity: 0.6; }
  .ring-b { fill: none; stroke: var(--mood); stroke-width: 7; opacity: 0.35; }
  .ring-b-major { fill: none; stroke: var(--mood-hi); stroke-width: 11; opacity: 0.8; }
  .wave { fill: none; stroke: var(--mood-hi); stroke-width: 1.6; stroke-linejoin: round; opacity: 0.9; }
  .ring-d { fill: none; stroke: var(--mood); stroke-width: 2.5; stroke-linecap: round; opacity: 0.85; }
  .ring-d2 { fill: none; stroke: var(--mood-hi); stroke-width: 4; opacity: 0.5; }
  .mote { fill: var(--mood-hi); }
  .gyro { fill: none; stroke: var(--mood); stroke-width: 1.3; }
  .gyro.alt { stroke: var(--mood-hi); stroke-dasharray: 3 5; }
  .eq { fill: none; stroke: var(--mood-hi); stroke-width: 2.2; stroke-linecap: round; }
  .bolt { fill: none; stroke: var(--mood-hi); stroke-width: 1.4; stroke-linejoin: round; }
  .blip circle { fill: var(--mood-hi); }
  .blip .blip-ring { fill: none; stroke: var(--mood); stroke-width: 1; }

  .core-shell { fill: none; stroke: var(--mood); stroke-width: 1; opacity: 0.6; }
  .reactor polygon { fill: none; stroke: var(--mood-hi); stroke-width: 1.4; stroke-linejoin: round; }
  .heart { fill: #ffffff; opacity: 0.95; }

  .ripple { fill: none; }

  .neural { stroke: var(--mood-hi); stroke-width: 1; }

  .sat-link { stroke: var(--sat); stroke-width: 0.8; stroke-dasharray: 2 5; opacity: 0.45; }
  .sat.pending .sat-link { animation: data-flow 0.5s linear infinite; }
  .sat-node { fill: var(--sat); }
  .sat-ring { fill: none; stroke: var(--sat); stroke-width: 1; opacity: 0.7; }
  .sat.pending .sat-ring { stroke-dasharray: 4 3; animation: sat-spin 1.2s linear infinite; }
  .sat-label {
    font: 700 8px ui-monospace, 'JetBrains Mono', monospace;
    letter-spacing: 1px;
    fill: var(--sat);
    paint-order: stroke;
    stroke: #060b18;
    stroke-width: 3px;
  }

  .reticle path { fill: none; stroke: var(--mood-hi); stroke-width: 1.4; stroke-linecap: round; }
  .reticle-dot { fill: var(--mood-hi); }
  .tether { stroke: var(--mood); stroke-width: 0.8; stroke-dasharray: 1 4; }
  .reticle-label text {
    font: 8px ui-monospace, 'JetBrains Mono', monospace;
    letter-spacing: 1px;
    fill: #9fb3d1;
    paint-order: stroke;
    stroke: #060b18;
    stroke-width: 3px;
  }
  .reticle-label .lock { fill: var(--mood-hi); font-weight: 700; letter-spacing: 2px; }

  .hud text {
    font-family: ui-monospace, 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 8.5px;
    letter-spacing: 1.2px;
    fill: #7f93b3;
    white-space: pre;
  }
  .hud .hud-title { fill: var(--mood-hi); font-weight: 700; letter-spacing: 2px; opacity: 0.85; }
  .hud .hud-val { fill: var(--mood); font-weight: 700; }
  .hud .hud-dim { opacity: 0.7; }
  .gauge-bg { fill: rgba(255, 255, 255, 0.08); }

  .banner {
    position: absolute;
    top: 22.6%;
    left: 16px;
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--halo);
    background: rgba(5, 10, 22, 0.85);
    color: var(--halo);
    font: 700 9px/1.4 ui-monospace, 'JetBrains Mono', monospace;
    letter-spacing: 1px;
    text-transform: uppercase;
    white-space: nowrap;
    box-shadow: 0 0 12px -2px var(--halo);
    animation: alarm 1.2s ease-in-out infinite;
    pointer-events: none;
  }

  .poke {
    position: absolute;
    width: 84px;
    height: 84px;
    border: none;
    border-radius: 50%;
    background: transparent;
    cursor: grab;
    touch-action: none;
    z-index: 2;
  }
  .poke:active { cursor: grabbing; }
  .poke:focus-visible,
  .key-toggle:focus-visible {
    outline: 2px solid var(--mood);
    outline-offset: 2px;
  }

  .key-toggle {
    position: absolute;
    right: 14px;
    bottom: 12px;
    z-index: 3;
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(5, 10, 22, 0.6);
    color: #7f93b3;
    font: 700 9px/1.6 ui-monospace, 'JetBrains Mono', monospace;
    letter-spacing: 1.5px;
    cursor: pointer;
  }
  .key-toggle:hover,
  .key-toggle.on {
    color: var(--mood);
    border-color: var(--mood);
  }

  .status-text {
    position: absolute;
    bottom: 12px;
    left: 0;
    right: 0;
    text-align: center;
    font: 700 11px/1.4 ui-monospace, 'JetBrains Mono', monospace;
    letter-spacing: 3px;
    text-transform: uppercase;
    white-space: nowrap;
    color: var(--mood-hi);
    text-shadow: 0 0 10px var(--mood), 0 1px 2px rgba(0, 0, 0, 0.8);
    pointer-events: none;
  }

  @keyframes data-flow {
    to { stroke-dashoffset: -7; }
  }
  @keyframes sat-spin {
    to { stroke-dashoffset: -14; }
  }
  @keyframes alarm {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }
  @keyframes stutter {
    0%, 100% { opacity: 0.8; }
    12% { opacity: 0.15; }
    18% { opacity: 0.7; }
    55% { opacity: 0.25; }
    60% { opacity: 0.8; }
  }
  @keyframes tick-crawl {
    to { stroke-dashoffset: -8.52; } /* two 2° tick periods at r=122 */
  }
  @keyframes dash-crawl {
    to { stroke-dashoffset: -48.97; } /* one 23° dash period at r=122 */
  }

  @media (prefers-reduced-motion: reduce) {
    .halo, .halo-rim, .chevron, .banner, .sat-link, .sat-ring {
      animation: none;
    }
  }
</style>
