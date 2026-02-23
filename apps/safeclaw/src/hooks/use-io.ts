import { getGatewayUrl } from "@/models/settings.model";
import { useReactive } from "ahooks";
import { useCallback, useEffect } from "react";
import { io, Socket } from "socket.io-client";

export const useIo = (namespace?: string) => {
	const state = useReactive<{
		socket: Socket | null;
		isConnected: boolean;
		isConnecting: boolean;
	}>({
		socket: null,
		isConnected: false,
		isConnecting: false,
	});

	// 连接 socket
	const connect = useCallback(() => {
		if (!state.socket) {
			state.isConnecting = true;
			state.socket = io(`${getGatewayUrl()}${namespace}`);
			state.socket.on("connect", () => {
				state.isConnected = true;
				state.isConnecting = false;
			});
			state.socket.on("exception", () => {
				state.isConnecting = false;
			});
			state.socket.on("disconnect", () => {
				state.isConnected = false;
			});
			state.socket.on("connect_error", () => {
				state.isConnecting = false;
			});
			state.socket.on("connect_timeout", () => {
				state.isConnecting = false;
			});
		}
	}, [namespace, state]);

	// 断开连接
	const disconnect = useCallback(() => {
		if (state.socket) {
			state.socket.disconnect();
			state.socket = null;
			state.isConnected = false;
		}
	}, [state]);

	// 重新连接
	const reconnect = useCallback(() => {
		disconnect();
		connect();
	}, [disconnect, connect]);

	// 事件监听
	const on = useCallback(
		(ev: string, listener: (...args: any[]) => void) => {
			if (state.socket) {
				state.socket.on(ev, listener);
			}
		},
		[state.socket],
	);

	// 发送事件
	const emit = useCallback(
		(ev: string, ...args: any[]) => {
			if (state.socket) {
				state.socket.emit(ev, ...args);
			}
		},
		[state.socket],
	);

	// 错误处理
	const onError = useCallback(
		(listener: (error: any) => void) => {
			if (state.socket) {
				state.socket.on("error", listener);
			}
		},
		[state.socket],
	);

	// 连接状态变化监听
	const onConnectionChange = useCallback(
		(listener: (isConnected: boolean) => void) => {
			if (state.socket) {
				state.socket.on("connect", () => listener(true));
				state.socket.on("disconnect", () => listener(false));
			}
		},
		[state.socket],
	);

	// 在组件挂载时自动连接
	useEffect(() => {
		connect();
		return disconnect;
	}, [connect, disconnect]);

	return {
		socket: state.socket,
		isConnected: state.isConnected,
		isConnecting: state.isConnecting,
		connect,
		disconnect,
		reconnect,
		on,
		emit,
		onError,
		onConnectionChange,
	};
};
