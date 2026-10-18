// Background: a two-source interference pattern drawn on a canvas. The two
// coherent point sources drift slowly; intensity is |e^{ik r1} + e^{ik r2}|^2.
// Purely decorative; respects prefers-reduced-motion.
export function startFringes(canvas: HTMLCanvasElement) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const scale = 0.25; // render at quarter resolution, upscale with CSS
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  let w = 0, h = 0, image: ImageData;
  const resize = () => {
    w = Math.max(1, Math.floor(window.innerWidth * scale));
    h = Math.max(1, Math.floor(window.innerHeight * scale));
    canvas.width = w;
    canvas.height = h;
    image = ctx.createImageData(w, h);
  };
  resize();
  window.addEventListener("resize", resize);
  const k = 0.42;
  const frame = (t: number) => {
    const s = t / 9000;
    const x1 = w * (0.3 + 0.08 * Math.sin(s)), y1 = h * (0.35 + 0.1 * Math.cos(s * 0.7));
    const x2 = w * (0.7 + 0.08 * Math.cos(s * 0.9)), y2 = h * (0.65 + 0.1 * Math.sin(s * 1.1));
    const data = image.data;
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const r1 = Math.hypot(x - x1, y - y1), r2 = Math.hypot(x - x2, y - y2);
        const phase = k * (r1 - r2);
        const intensity = 0.5 + 0.5 * Math.cos(phase); // normalized |E1+E2|^2
        const fade = Math.exp(-(r1 + r2) / (w * 0.9));
        const v = intensity * fade;
        const i = (y * w + x) * 4;
        data[i] = 10 * v;
        data[i + 1] = 90 * v;
        data[i + 2] = 255 * v;
        data[i + 3] = 255;
      }
    }
    ctx.putImageData(image, 0, 0);
    if (!reduce) requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}
