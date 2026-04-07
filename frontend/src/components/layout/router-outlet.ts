/**
 * Router outlet — a plain container for route-based content.
 * NOT a BaseComponent to avoid re-rendering (which would overwrite router-mounted pages).
 * Shows a loading spinner initially; the router replaces it with the matched page component.
 */

// Safe: all content below is static developer-controlled HTML, no user input
// nosemgrep: javascript.browser.security.insecure-document-method.insecure-document-method
class RouterOutlet extends HTMLElement {
  connectedCallback(): void {
    const spinner = document.createElement('div');
    spinner.style.cssText = 'display:flex;flex-direction:column;align-items:center;justify-content:center;padding:4rem 2rem;color:var(--text-color-muted);gap:1.25rem;min-height:200px';

    const circle = document.createElement('div');
    circle.style.cssText = 'width:48px;height:48px;border:3px solid var(--border-glass,#333);border-top-color:var(--color-primary,#5d9cec);border-radius:50%;animation:spin .8s linear infinite';

    const label = document.createElement('span');
    label.style.cssText = 'font-size:.875rem;font-weight:500';
    label.textContent = 'Loading...';

    const style = document.createElement('style');
    style.textContent = '@keyframes spin{to{transform:rotate(360deg)}}';

    spinner.append(circle, label);
    this.append(spinner, style);
  }
}

if (!customElements.get('router-outlet')) {
  customElements.define('router-outlet', RouterOutlet);
}
