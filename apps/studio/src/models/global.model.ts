import { proxy } from "valtio";

const globalModel = proxy({
	sidecarConnected: false,
});

export default globalModel;
