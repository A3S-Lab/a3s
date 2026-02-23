import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Input } from "@/components/ui/input";
import { DialogProps } from "@radix-ui/react-dialog";
import { useReactive } from "ahooks";
import { createContext, useContext, useRef, useEffect } from "react";
import { Button } from "../ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "../ui/dialog";

type Modal = "alert" | "dialog" | "prompt";

type ModalProps = {
	title: React.ReactNode;
	description?: React.ReactNode;
	cancelText?: React.ReactNode;
	confirmText?: React.ReactNode;
	onCancel?: () => void | Promise<void>;
	onConfirm?: () => void | boolean | Promise<void> | Promise<boolean>;
};

type PromptProps = {
	title: React.ReactNode;
	description?: React.ReactNode;
	defaultValue?: string;
	placeholder?: string;
	cancelText?: React.ReactNode;
	confirmText?: React.ReactNode;
	onCancel?: () => void | Promise<void>;
	onConfirm?: (
		value: string,
	) => void | boolean | Promise<void> | Promise<boolean>;
};

const initialModalProps: ModalProps = {
	title: "",
	description: "",
	cancelText: "取消",
	confirmText: "确定",
	onCancel: () => {},
	onConfirm: () => true,
};

const initialPromptProps: PromptProps = {
	title: "",
	description: "",
	defaultValue: "",
	placeholder: "",
	cancelText: "取消",
	confirmText: "确定",
	onCancel: () => {},
	onConfirm: () => true,
};

type ModalProviderProps = {
	children: React.ReactNode;
	defaultType?: Modal;
};

type ModalProviderState = DialogProps & {
	alert: (props: ModalProps) => void;
	dialog: (props: ModalProps) => void;
	prompt: (props: PromptProps) => void;
};

const initialState: ModalProviderState = {
	open: false,
	alert: () => {},
	dialog: () => {},
	prompt: () => {},
};

const ModalProviderContext = createContext<ModalProviderState>(initialState);

function PromptInput({
	defaultValue,
	placeholder,
	inputRef,
}: {
	defaultValue?: string;
	placeholder?: string;
	inputRef: React.RefObject<HTMLInputElement | null>;
}) {
	useEffect(() => {
		// Auto-focus and select on mount
		setTimeout(() => {
			inputRef.current?.focus();
			inputRef.current?.select();
		}, 0);
	}, [inputRef]);

	return (
		<Input
			ref={inputRef}
			defaultValue={defaultValue}
			placeholder={placeholder}
			className="mt-2"
		/>
	);
}

export function ModalProvider({ children, defaultType }: ModalProviderProps) {
	const promptInputRef = useRef<HTMLInputElement | null>(null);

	const state = useReactive<
		ModalProviderState & {
			type: Modal;
			modalProps: ModalProps;
			promptProps: PromptProps;
		}
	>({
		open: false,
		type: defaultType ?? "alert",
		modalProps: {
			title: "",
		},
		promptProps: {
			title: "",
		},
		alert: (props: ModalProps) => {
			state.type = "alert";
			state.modalProps = { ...initialModalProps, ...props };
			state.open = true;
		},
		dialog: (props: ModalProps) => {
			state.type = "dialog";
			state.modalProps = { ...initialModalProps, ...props };
			state.open = true;
		},
		prompt: (props: PromptProps) => {
			state.type = "prompt";
			state.promptProps = { ...initialPromptProps, ...props };
			state.open = true;
		},
	});

	return (
		<ModalProviderContext.Provider
			value={{
				open: state.open,
				alert: state.alert,
				dialog: state.dialog,
				prompt: state.prompt,
			}}
		>
			{children}
			{state.type === "alert" && (
				<AlertDialog open={state.open}>
					<AlertDialogContent>
						<AlertDialogHeader>
							<AlertDialogTitle>{state.modalProps.title}</AlertDialogTitle>
							{state.modalProps?.description && (
								<AlertDialogDescription>
									{state.modalProps?.description}
								</AlertDialogDescription>
							)}
						</AlertDialogHeader>
						<AlertDialogFooter>
							<AlertDialogCancel
								onClick={() => {
									state.modalProps?.onCancel?.();
									state.open = false;
								}}
							>
								{state.modalProps?.cancelText}
							</AlertDialogCancel>
							<AlertDialogAction
								onClick={async () => {
									const isConfirm = await state.modalProps?.onConfirm?.();
									if (typeof isConfirm === "undefined") {
										state.open = false;
									} else {
										state.open = !isConfirm;
									}
								}}
							>
								{state.modalProps?.confirmText}
							</AlertDialogAction>
						</AlertDialogFooter>
					</AlertDialogContent>
				</AlertDialog>
			)}
			{state.type === "dialog" && (
				<Dialog open={state.open} onOpenChange={(open) => (state.open = open)}>
					<DialogContent>
						<DialogHeader>
							<DialogTitle>{state.modalProps.title}</DialogTitle>
							{state.modalProps?.description && (
								<DialogDescription>
									{state.modalProps?.description}
								</DialogDescription>
							)}
						</DialogHeader>
						<DialogFooter>
							<Button
								onClick={async () => {
									await state.modalProps?.onConfirm?.();
									state.open = false;
								}}
							>
								{state.modalProps?.confirmText}
							</Button>
						</DialogFooter>
					</DialogContent>
				</Dialog>
			)}
			{state.type === "prompt" && (
				<AlertDialog open={state.open}>
					<AlertDialogContent>
						<AlertDialogHeader>
							<AlertDialogTitle>{state.promptProps.title}</AlertDialogTitle>
							{state.promptProps?.description && (
								<AlertDialogDescription>
									{state.promptProps?.description}
								</AlertDialogDescription>
							)}
						</AlertDialogHeader>
						<PromptInput
							defaultValue={state.promptProps.defaultValue}
							placeholder={state.promptProps.placeholder}
							inputRef={promptInputRef}
						/>
						<AlertDialogFooter>
							<AlertDialogCancel
								onClick={() => {
									state.promptProps?.onCancel?.();
									state.open = false;
								}}
							>
								{state.promptProps?.cancelText}
							</AlertDialogCancel>
							<AlertDialogAction
								onClick={async () => {
									const value = promptInputRef.current?.value ?? "";
									const isConfirm = await state.promptProps?.onConfirm?.(value);
									if (typeof isConfirm === "undefined") {
										state.open = false;
									} else {
										state.open = !isConfirm;
									}
								}}
							>
								{state.promptProps?.confirmText}
							</AlertDialogAction>
						</AlertDialogFooter>
					</AlertDialogContent>
				</AlertDialog>
			)}
		</ModalProviderContext.Provider>
	);
}

export const useModal = () => {
	const context = useContext(ModalProviderContext);

	if (context === undefined)
		throw new Error("useModal must be used within a ModalProviderContext");

	return context;
};
