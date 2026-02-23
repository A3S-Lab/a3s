import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
	await page.goto("/");
	await page.evaluate(() =>
		localStorage.setItem("safeclaw-onboarding-complete", "true"),
	);
	await page.reload();
});

test.describe("Archive page", () => {
	test.beforeEach(async ({ page }) => {
		await page.getByRole("tab", { name: "Archive" }).click();
		await expect(page).toHaveURL(/#\/archive/);
	});

	test("shows page header", async ({ page }) => {
		await expect(page.getByRole("heading", { name: "归档记录" })).toBeVisible();
	});

	test("shows empty state when no archived sessions", async ({ page }) => {
		await expect(page.getByText("暂无归档会话")).toBeVisible();
	});

	test("shows search input", async ({ page }) => {
		await expect(page.getByPlaceholder("搜索归档...")).toBeVisible();
	});

	test("search with no match shows empty state", async ({ page }) => {
		await page.getByPlaceholder("搜索归档...").fill("zzz_no_match_xyz");
		await expect(page.getByText("未找到匹配的归档")).toBeVisible();
	});

	test("shows placeholder when no session selected", async ({ page }) => {
		await expect(page.getByText("选择一个归档会话查看记录")).toBeVisible();
	});

	test("archive nav item is active", async ({ page }) => {
		await expect(page.getByRole("tab", { name: "Archive" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
	});
});
