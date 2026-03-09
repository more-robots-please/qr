let debounceTimer = null;
let currentSvg = null;
let currentPngUrl = null;

const urlInput = document.getElementById('url');
const logoToggle = document.getElementById('logo');
const printToggle = document.getElementById('print-mode');
const container = document.getElementById('qr-container');
const modeLabel = document.getElementById('mode-label');
const dlPng = document.getElementById('dl-png');
const dlSvg = document.getElementById('dl-svg');

function onInput() {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(generate, 400);
}

async function generate() {
  const url = urlInput.value.trim();
  if (!url) {
    container.innerHTML = '<span>enter a url to preview</span>';
    currentSvg = null;
    dlPng.disabled = true;
    dlSvg.disabled = true;
    return;
  }

  const print = printToggle.checked;
  const logo = logoToggle.checked;

  container.innerHTML = '<span>generating...</span>';
  dlPng.disabled = true;
  dlSvg.disabled = true;

  const res = await fetch('/api/generate', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url, logo, print_mode: print })
  });

  if (!res.ok) {
    container.innerHTML = '<span style="color:#ff6b6b">invalid url</span>';
    return;
  }

  const data = await res.json();
  currentSvg = data.svg;
  currentPngUrl = `/api/png?url=${encodeURIComponent(url)}&logo=${logo}&print_mode=${print}`;

  const blob = new Blob([currentSvg], { type: 'image/svg+xml' });
  const objUrl = URL.createObjectURL(blob);
  container.innerHTML = `<img src="${objUrl}" alt="QR code" />`;
  modeLabel.textContent = print ? 'print mode — black on white' : 'screen mode — pink on black';

  dlPng.disabled = false;
  dlSvg.disabled = false;
}

async function downloadPng() {
  const url = urlInput.value.trim();
  if (!url) return;
  const print = printToggle.checked;
  const logo = logoToggle.checked;
  const res = await fetch(`/api/png?url=${encodeURIComponent(url)}&logo=${logo}&print_mode=${print}`);
  const blob = await res.blob();
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'qr.png';
  a.click();
}

function downloadSvg() {
  if (!currentSvg) return;
  const blob = new Blob([currentSvg], { type: 'image/svg+xml' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'qr.svg';
  a.click();
}

urlInput.addEventListener('input', onInput);
logoToggle.addEventListener('change', generate);
printToggle.addEventListener('change', generate);

// Auto-generate if URL param provided
const params = new URLSearchParams(window.location.search);
if (params.get('url')) {
  urlInput.value = params.get('url');
  generate();
}
