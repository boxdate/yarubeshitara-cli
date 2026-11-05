// Theme Switcher for Yarubeshitara Documentation Site
const THEMES = {
  'tokyo-night': {
    command: 'yarube theme tokyo-night',
    cssClass: 'theme-tokyo-night'
  },
  'monokai': {
    command: 'yarube theme monokai',
    cssClass: 'theme-monokai'
  },
  'nord': {
    command: 'yarube theme nord',
    cssClass: 'theme-nord'
  },
  'sepia': {
    command: 'yarube theme sepia',
    cssClass: 'theme-sepia'
  }
};

function selectTheme(themeKey) {
  const theme = THEMES[themeKey];
  if (!theme) return;

  // Update active tab button
  document.querySelectorAll('.theme-tab').forEach((btn) => {
    const isSelected = btn.getAttribute('data-theme') === themeKey;
    btn.classList.toggle('active', isSelected);
    btn.setAttribute('aria-selected', isSelected ? 'true' : 'false');
  });

  // Update terminal preview class
  const previewPane = document.getElementById('theme-terminal-pane');
  if (previewPane) {
    previewPane.className = `theme-terminal-pane ${theme.cssClass}`;
  }

  // Update command string inside preview
  const cliLine = document.getElementById('theme-cli-line');
  if (cliLine) {
    cliLine.textContent = theme.command;
  }
}

function copyCode(sourceId, buttonEl) {
  const el = document.getElementById(sourceId);
  if (!el) return;

  const rawText = el.getAttribute('data-copy') || el.innerText.trim();
  navigator.clipboard.writeText(rawText).then(() => {
    const prev = buttonEl.textContent;
    buttonEl.textContent = 'Copied';
    buttonEl.style.color = '#8da183';
    buttonEl.style.borderColor = '#8da183';
    setTimeout(() => {
      buttonEl.textContent = prev;
      buttonEl.style.color = '';
      buttonEl.style.borderColor = '';
    }, 1500);
  });
}

document.addEventListener('DOMContentLoaded', () => {
  selectTheme('tokyo-night');
});
