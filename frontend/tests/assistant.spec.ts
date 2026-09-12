import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * The org-scoped "ask your data" assistant is a global widget, available on
 * every page. These tests cover the deterministic UI flow (open, ask, close)
 * without depending on a live Anthropic call — the same convention the
 * existing AI dispatch-summary feature follows: nothing in this repo's
 * Playwright suite exercises a real LLM response, since ANTHROPIC_API_KEY
 * isn't configured in CI. The "nothing indexed yet" canned answer is fully
 * deterministic (the backend returns it before ever calling Claude), so it's
 * the one path safe to assert on end-to-end.
 */
test.describe('AI assistant widget', () => {
  test('is collapsed by default and opens on click', async ({ page }) => {
    await registerOrg(page, `Assistant Widget ${uid()}`);

    await expect(page.getByRole('dialog', { name: /ask your data/i })).not.toBeVisible();
    await page.getByRole('button', { name: /open assistant/i }).click();
    await expect(page.getByRole('dialog', { name: /ask your data/i })).toBeVisible();
  });

  test('answers with a canned response when nothing is indexed yet, and is available from any page', async ({ page }) => {
    await registerOrg(page, `Assistant Empty Index ${uid()}`);

    await page.getByRole('button', { name: /open assistant/i }).click();
    await page.getByLabel(/question for the assistant/i).fill('what happened to my dispatches?');
    await page.getByRole('button', { name: /^ask$/i }).click();

    await expect(page.getByText(/indexed yet/i)).toBeVisible({ timeout: 10000 });

    // Navigate away via an in-app link (a real page.goto would reload the
    // whole SPA and reset React state, which isn't what "persists across
    // pages" means) — the widget lives outside the routed page content.
    await page.getByRole('link', { name: 'Customers' }).click();
    await expect(page).toHaveURL(/\/customers$/);
    await expect(page.getByRole('button', { name: /close assistant/i })).toBeVisible();
    await expect(page.getByText(/indexed yet/i)).toBeVisible();
  });

  test('closes when the close button is clicked', async ({ page }) => {
    await registerOrg(page, `Assistant Close ${uid()}`);

    await page.getByRole('button', { name: /open assistant/i }).click();
    await expect(page.getByRole('dialog', { name: /ask your data/i })).toBeVisible();
    await page.getByRole('button', { name: /close assistant/i }).click();
    await expect(page.getByRole('dialog', { name: /ask your data/i })).not.toBeVisible();
  });
});
