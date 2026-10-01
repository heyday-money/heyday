// Apply before first paint, including in the native WebView's self-only CSP.
try {
  const saved = localStorage.getItem('theme');
  document.documentElement.dataset.theme = ['light', 'dark', 'heyday'].includes(saved) ? saved : 'heyday';
} catch { document.documentElement.dataset.theme = 'heyday'; }

try { document.documentElement.lang = localStorage.getItem("heyday-language") === "th" ? "th" : "en"; } catch { document.documentElement.lang = "en"; }
