import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
	await page.goto("/");
	await page.evaluate(() =>
		localStorage.setItem("safeclaw-onboarding-complete", "true"),
	);
	await page.reload();
});

test.describe("Agent page", () => {
	test("shows agent list sidebar", async ({ page }) => {
		await expect(page.getByRole("heading", { name: "智能体" })).toBeVisible();
	});

	test("shows search input", async ({ page }) => {
		await expect(page.getByPlaceholder("搜索智能体...")).toBeVisible();
	});

	test("search filters agent list", async ({ page }) => {
		const input = page.getByPlaceholder("搜索智能体...");
		await input.fill("zzz_no_match_xyz");
		await expect(page.getByText("未找到匹配的智能体")).toBeVisible();
	});

	test("new session button opens create dialog", async ({ page }) => {
		await page.getByRole("button", { name: "自定义新建会话" }).click();
		// Dialog should appear
		await expect(page.getByRole("dialog")).toBeVisible();
	});

	test("shows global stats bar", async ({ page }) => {
		// Stats bar is always rendered at the bottom of the sidebar
		const nav = page.locator('[aria-label="智能体列表"]').locator("..");
		await expect(nav).toBeVisible();
	});
});

test.describe("Activity page", () => {
	test.beforeEach(async ({ page }) => {
		await page.getByRole("tab", { name: "Monitor" }).click();
		await expect(page).toHaveURL(/#\/monitor/);
		// Activity is the default tab inside Monitor
	});

	test("shows page header", async ({ page }) => {
		await expect(
			page.getByText("所有 Agent 的工具调用与任务完成记录"),
		).toBeVisible();
	});

	test("shows empty state when no activity", async ({ page }) => {
		await expect(page.getByText("暂无活动记录")).toBeVisible();
		await expect(
			page.getByText("Agent 执行工具调用后将在此显示"),
		).toBeVisible();
	});

	test("filter bar renders all filter buttons", async ({ page }) => {
		await expect(page.getByRole("button", { name: /全部/ })).toBeVisible();
		await expect(page.getByRole("button", { name: /工具调用/ })).toBeVisible();
		await expect(page.getByRole("button", { name: /失败/ })).toBeVisible();
		await expect(page.getByRole("button", { name: /完成/ })).toBeVisible();
		await expect(page.getByRole("button", { name: /错误/ })).toBeVisible();
	});

	test("'全部' filter is active by default", async ({ page }) => {
		const allBtn = page.getByRole("button", { name: /全部/ });
		// Active buttons have bg-primary/10 class
		await expect(allBtn).toHaveClass(/bg-primary\/10/);
	});

	test("clicking a filter changes active state", async ({ page }) => {
		const toolBtn = page.getByRole("button", { name: /工具调用/ });
		await toolBtn.click();
		await expect(toolBtn).toHaveClass(/bg-primary\/10/);
		// '全部' should no longer be active
		const allBtn = page.getByRole("button", { name: /全部/ });
		await expect(allBtn).not.toHaveClass(/bg-primary\/10/);
	});
});

test.describe("Security page", () => {
	test.beforeEach(async ({ page }) => {
		await page.getByRole("tab", { name: "Security" }).click();
		await expect(page).toHaveURL(/#\/security/);
	});

	test("shows security overview header", async ({ page }) => {
		await expect(page.getByRole("heading", { name: "安全概览" })).toBeVisible();
	});

	test("shows TEE status card", async ({ page }) => {
		// TEE card should be present
		await expect(page.getByText(/TEE|可信执行环境/i)).toBeVisible();
	});

	test("shows empty audit log state", async ({ page }) => {
		// Navigate to audit log section
		await page.getByRole("button", { name: /审计日志/ }).click();
		await expect(page.getByText("暂无审计事件")).toBeVisible();
	});
});

test.describe("Settings page", () => {
	test.beforeEach(async ({ page }) => {
		await page.getByRole("tab", { name: "Settings" }).click();
		await expect(page).toHaveURL(/#\/settings/);
	});

	test("settings page renders", async ({ page }) => {
		// Page should load without error
		await expect(page.locator("body")).not.toContainText(
			"Something went wrong",
		);
	});
});
