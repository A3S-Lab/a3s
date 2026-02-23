import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
	await page.goto("/");
	await page.evaluate(() =>
		localStorage.setItem("safeclaw-onboarding-complete", "true"),
	);
	await page.reload();
});

test.describe("Settings — Agent section", () => {
	test.beforeEach(async ({ page }) => {
		await page.getByRole("tab", { name: "Settings" }).click();
		await expect(page).toHaveURL(/#\/settings/);
		// Click the sidebar nav button for Agent 配置
		await page
			.getByRole("navigation", { name: "Settings sections" })
			.getByRole("button", { name: /Agent 配置/ })
			.click();
	});

	test("shows agent defaults section", async ({ page }) => {
		// Section heading (h2)
		await expect(
			page.getByRole("heading", { name: "Agent 配置" }),
		).toBeVisible();
		await expect(page.getByPlaceholder("/path/to/workspace")).toBeVisible();
	});

	test("saves agent defaults", async ({ page }) => {
		await page.getByPlaceholder("0").first().fill("50");
		await page.getByRole("button", { name: "保存" }).click();
		await expect(page.getByText("Agent 配置已保存")).toBeVisible();
	});
});

test.describe("Settings — Provider health check", () => {
	test.beforeEach(async ({ page }) => {
		await page.getByRole("tab", { name: "Settings" }).click();
		await expect(page).toHaveURL(/#\/settings/);
	});

	test("shows test connection button for each provider", async ({ page }) => {
		// aria-label is "测试 <name> 连接"
		await expect(
			page.getByRole("button", { name: /测试.*连接/ }).first(),
		).toBeVisible();
	});

	test("test connection button changes state on click", async ({ page }) => {
		const testBtn = page.getByRole("button", { name: /测试.*连接/ }).first();
		await testBtn.click();
		// Should show testing or result state
		await expect(page.getByText(/测试中|ms|失败/)).toBeVisible({
			timeout: 8000,
		});
	});
});
