import { proxy } from "valtio";

const settingsModel = proxy({
	sidecarUrl: "http://127.0.0.1:3000",
});

export default settingsModel;
