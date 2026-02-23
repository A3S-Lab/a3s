import { test, expect } from "@playwright/test";

// Skip onboarding by setting the localStorage flag before each test
test.beforeEach(async ({ page }) => {
	await page.goto("/");
	await page.evaluate(() => {
		localStorage.setItem("safeclaw-onboarding-complete", "true");
	});
	await page.reload();
});

test.describe("Activity bar navigation", () => {
	test("renders all nav items", async ({ page }) => {
		const nav = page.getByRole("navigation", { name: "Main navigation" });
		await expect(nav).toBeVisible();
		await expect(nav.getByRole("tab", { name: "Chat" })).toBeVisible();
		await expect(nav.getByRole("tab", { name: "Monitor" })).toBeVisible();
		await expect(nav.getByRole("tab", { name: "Security" })).toBeVisible();
		await expect(nav.getByRole("tab", { name: "Archive" })).toBeVisible();
		await expect(nav.getByRole("tab", { name: "Settings" })).toBeVisible();
	});

	test("navigates to Monitor page", async ({ page }) => {
		await page.getByRole("tab", { name: "Monitor" }).click();
		await expect(page).toHaveURL(/#\/monitor/);
		await expect(page.getByRole("tab", { name: "Monitor" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
	});

	test("navigates to Security page", async ({ page }) => {
		await page.getByRole("tab", { name: "Security" }).click();
		await expect(page).toHaveURL(/#\/security/);
		await expect(page.getByRole("tab", { name: "Security" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
	});

	test("navigates to Archive page", async ({ page }) => {
		await page.getByRole("tab", { name: "Archive" }).click();
		await expect(page).toHaveURL(/#\/archive/);
	});

	test("navigates to Settings page", async ({ page }) => {
		await page.getByRole("tab", { name: "Settings" }).click();
		await expect(page).toHaveURL(/#\/settings/);
	});

	test("keyboard ArrowDown moves focus between nav items", async ({ page }) => {
		const chatTab = page.getByRole("tab", { name: "Chat" });
		await chatTab.focus();
		await page.keyboard.press("ArrowDown");
		await expect(page.getByRole("tab", { name: "Monitor" })).toBeFocused();
	});

	test("keyboard ArrowUp wraps around", async ({ page }) => {
		const chatTab = page.getByRole("tab", { name: "Chat" });
		await chatTab.focus();
		await page.keyboard.press("ArrowUp");
		// wraps to last item (Settings)
		await expect(page.getByRole("tab", { name: "Settings" })).toBeFocused();
	});
});
