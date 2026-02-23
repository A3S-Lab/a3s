const isDev = process.env.NODE_ENV === "development";

export default {
	isDev,
	name: "SafeClaw",
	description: "Secure Personal AI Assistant with TEE Support",
	gatewayUrl: import.meta.env.PUBLIC_GATEWAY_URL || "http://127.0.0.1:18790",
	localStorageKeyPrefix: "safeclaw",
};
