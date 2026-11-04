// Tracks in-flight backend calls. The visual verifier waits for
// `data-busy="0"` on <html> before capturing, so screenshots never show a
// half-loaded state.
let pending = 0;
const mark = () => {
  document.documentElement.dataset.busy = pending > 0 ? "1" : "0";
};

export async function track<T>(p: Promise<T>): Promise<T> {
  pending++;
  mark();
  try {
    return await p;
  } finally {
    pending--;
    // Let React commit the resulting state before reporting idle.
    setTimeout(mark, 0);
  }
}
