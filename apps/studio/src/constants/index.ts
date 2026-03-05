const isDev = process.env.NODE_ENV === "development";

export default {
	isDev,
	name: "Studio",
	description: "One-person Company Workstation",
	sidecarUrl: import.meta.env.PUBLIC_SIDECAR_URL || "http://127.0.0.1:3000",
	localStorageKeyPrefix: "studio",
};
