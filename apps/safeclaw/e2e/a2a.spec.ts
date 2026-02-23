import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
	await page.goto("/");
	await page.evaluate(() =>
		localStorage.setItem("safeclaw-onboarding-complete", "true"),
	);
	await page.reload();
});

test.describe("Monitor page", () => {
	test.beforeEach(async ({ page }) => {
		await page
			.getByRole("navigation", { name: "Main navigation" })
			.getByRole("tab", { name: "Monitor" })
			.click();
		await expect(page).toHaveURL(/#\/monitor/);
	});

	test("shows Activity tab by default", async ({ page }) => {
		await expect(page.getByRole("tab", { name: "活动" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await expect(page.getByText("暂无活动记录")).toBeVisible();
	});

	test("switches to A2A tab", async ({ page }) => {
		await page.locator("button[role='tab']").filter({ hasText: "A2A" }).click();
		// A2A tab shows either the routing graph or empty state
		await expect(
			page
				.getByRole("heading", { name: "A2A 路由图" })
				.or(page.getByText("暂无 A2A 通信记录")),
		).toBeVisible();
	});

	test("switches to Events tab", async ({ page }) => {
		await page.getByRole("tab", { name: "事件" }).click();
	});

	test("Monitor nav item is active", async ({ page }) => {
		await expect(
			page
				.getByRole("navigation", { name: "Main navigation" })
				.getByRole("tab", { name: "Monitor" }),
		).toHaveAttribute("aria-selected", "true");
	});
});
