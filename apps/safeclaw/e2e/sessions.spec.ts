import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
	await page.goto("/");
	await page.evaluate(() =>
		localStorage.setItem("safeclaw-onboarding-complete", "true"),
	);
	await page.reload();
});

test.describe("Session config drawer", () => {
	test("gear button opens config drawer", async ({ page }) => {
		// Need an active session — skip if none available
		const agentList = page.getByRole("listbox", { name: "智能体列表" });
		const firstAgent = agentList.locator("[role=option]").first();
		const count = await firstAgent.count();
		if (count === 0) {
			test.skip();
			return;
		}
		await firstAgent.click();
		// Config gear button should be visible in chat header
		const gearBtn = page.getByRole("button", { name: "会话配置" });
		await expect(gearBtn).toBeVisible();
		await gearBtn.click();
		// Drawer should open
		await expect(page.getByRole("dialog")).toBeVisible();
		await expect(page.getByText("会话配置")).toBeVisible();
	});
});
