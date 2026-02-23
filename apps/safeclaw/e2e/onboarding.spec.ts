import { test, expect } from "@playwright/test";

test.describe("Onboarding wizard", () => {
	test.beforeEach(async ({ page }) => {
		// Clear onboarding flag so wizard shows
		await page.goto("/");
		await page.evaluate(() =>
			localStorage.removeItem("safeclaw-onboarding-complete"),
		);
		await page.reload();
	});

	test("shows wizard on first visit", async ({ page }) => {
		// Wizard dialog should be visible
		await expect(page.getByText("欢迎使用 SafeClaw")).toBeVisible();
		// Step indicator: 1 / 4
		await expect(page.getByText(/1\s*\/\s*4/)).toBeVisible();
	});

	test("can advance through welcome step", async ({ page }) => {
		await expect(page.getByText("欢迎使用 SafeClaw")).toBeVisible();
		await page.getByRole("button", { name: "下一步" }).click();
		// Now on gateway step (step 2)
		await expect(page.getByText(/2\s*\/\s*4/)).toBeVisible();
	});

	test("skip button dismisses wizard", async ({ page }) => {
		// Skip only appears on non-welcome steps — advance to gateway first
		await page.getByRole("button", { name: "下一步" }).click();
		await page.getByRole("button", { name: "跳过" }).click();
		await expect(page.getByText("欢迎使用 SafeClaw")).not.toBeVisible();
	});

	test("wizard is dismissed after completion", async ({ page }) => {
		// Step through all 4 steps
		for (let i = 0; i < 3; i++) {
			await page.getByRole("button", { name: "下一步" }).click();
		}
		// On done step — finish button
		await page.getByRole("button", { name: "开始使用" }).click();
		// Wizard should be gone
		await expect(page.getByText("欢迎使用 SafeClaw")).not.toBeVisible();
		// localStorage flag set
		const flag = await page.evaluate(() =>
			localStorage.getItem("safeclaw-onboarding-complete"),
		);
		expect(flag).toBe("true");
	});
});
