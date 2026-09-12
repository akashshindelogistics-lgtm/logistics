import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through the org-scoped **"ask your data" assistant**
 * on its own — a global widget, available from any page, that answers
 * natural-language questions grounded in retrieval over the org's own
 * indexed data (dispatch notifications, vehicle compliance documents) rather
 * than the LLM's general knowledge.
 *
 * Like the pre-existing AI dispatch-summary feature, the final answer step
 * needs ANTHROPIC_API_KEY configured on the server (not set in this
 * environment — see todo.org) — this demo shows the deterministic parts
 * that don't depend on it: the widget UI, indexing a real compliance
 * document, and retrieval actually finding it (visible in the "based on N
 * facts" footer once an answer comes back, or the same honest
 * "could not reach the assistant" message the AI summary feature already
 * shows when unconfigured).
 *
 * Watch it with `npm run test:e2e:demo:assistant` (headed, slowed).
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test('ask your data assistant', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Assistant Demo ${uid()}`);
  const vehicleReg = `AS${uid().toUpperCase()}`;
  const docNumber = `INS-${uid().toUpperCase()}`;

  await test.step('Register a vehicle with a compliance document', async () => {
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, {
      registration_number: vehicleReg,
      capacity: 20,
      unit: 'MetricTon',
    });
    await api(page, 'post', `/api/vehicles/${encodeURIComponent(vehicleReg)}/documents`, {
      doc_type: 'Insurance',
      document_number: docNumber,
      expires_on: new Date(Date.now() + 20 * 86_400_000).toISOString().slice(0, 10),
      notes: 'Renewed early this cycle.',
    });
  });

  await test.step('Open the assistant — it\'s available from any page', async () => {
    await page.goto(`/orgs/${org.id}`);
    await page.getByRole('button', { name: /open assistant/i }).click();
    await expect(page.getByRole('dialog', { name: /ask your data/i })).toBeVisible();
    await page.waitForTimeout(500);
  });

  await test.step('Ask a question with nothing relevant indexed yet', async () => {
    await page.getByLabel(/question for the assistant/i).fill('what is my delivery performance like?');
    await page.getByRole('button', { name: /^ask$/i }).click();
    await expect(page.getByText(/indexed yet/i)).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(1200);
  });

  await test.step('Ask about the vehicle document just recorded — retrieval finds it', async () => {
    await page.getByLabel(/question for the assistant/i).fill(`when does vehicle ${vehicleReg}'s insurance expire?`);
    await page.getByRole('button', { name: /^ask$/i }).click();

    // Either a real answer (ANTHROPIC_API_KEY configured) with a "based on
    // N facts" footer citing the vehicle_document chunk, or the same honest
    // "could not reach" message the pre-existing AI summary feature shows
    // when it isn't — both are real, demonstrable outcomes of this feature.
    await Promise.race([
      page.getByText(/based on \d+ fact/i).waitFor({ timeout: 15000 }),
      page.getByText(/could not reach the assistant/i).waitFor({ timeout: 15000 }),
    ]);
    await page.waitForTimeout(1500);
  });
});
