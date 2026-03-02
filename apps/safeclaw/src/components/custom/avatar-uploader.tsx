import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { getCroppedImg } from "@/lib/image";
import { cn } from "@/lib/utils";
import { AvatarProps } from "@radix-ui/react-avatar";
import { useReactive } from "ahooks";
import Compressor from "compressorjs";
import { ImageUpIcon } from "lucide-react";
import { forwardRef, useImperativeHandle, type ReactNode } from "react";
import { useDropzone } from "react-dropzone";
import Cropper from "react-easy-crop";
import { Avatar, AvatarFallback, AvatarImage } from "../ui/avatar";
import { Button } from "../ui/button";

type AvatarUploaderRefProps = {};

interface AvatarUploaderProps {
	className?: string;
	value?: string;
	onChange?: (value: string) => void;
	onUpload: (file: File) => Promise<string>;
	icon?: ReactNode;
	dropProps?: Record<string, unknown>;
	cropProps?: Record<string, unknown>;
	compressorProps?: Record<string, unknown>;
	avatarProps?: Partial<AvatarProps>;
}

const AvatarUploader = forwardRef<AvatarUploaderRefProps, AvatarUploaderProps>(
	(
		{
			onUpload,
			icon = <ImageUpIcon size={24} />,
			dropProps = {},
			cropProps,
			compressorProps,
			avatarProps,
			className,
			value,
			onChange,
		}: AvatarUploaderProps,
		ref,
	) => {
		const state = useReactive<{
			cropUrl: string | undefined;
			crop: { x: number; y: number };
			zoom: number;
			croppedAreaPixels: unknown;
		}>({
			cropUrl: undefined,
			crop: {
				x: 0,
				y: 0,
			},
			zoom: 1,
			croppedAreaPixels: null,
		});

		const { getRootProps, getInputProps } = useDropzone({
			accept: {
				"image/jpeg": [".jpg", ".jpeg"],
				"image/png": [".png"],
			},
			...dropProps,
			maxFiles: 1,
			maxSize: 10 * 1024 * 1024,
			multiple: false,
			onDrop: async (files: File[]) => {
				let file = files[0];
				if (compressorProps) {
					file = await new Promise(
						(resolve, reject) =>
							new Compressor(file, {
								...compressorProps,
								success(result: File) {
									resolve(result);
								},
								error(err: unknown) {
									reject(err);
								},
							}),
					);
				}
				if (cropProps) {
					state.cropUrl = await new Promise((resolve) => {
						const reader = new FileReader();
						reader.onload = (e) => {
							resolve(e.target?.result as string);
						};
						reader.readAsDataURL(file);
					});
					return;
				}
				const url = await onUpload(file);
				onChange?.(url);
			},
		});

		useImperativeHandle(ref, () => ({}), []);

		return (
			<div className={cn("group relative h-10 w-10 rounded-full", className)}>
				<Avatar
					className={cn("h-full w-full", avatarProps?.className)}
					{...avatarProps}
				>
					<AvatarImage src={value} />
					<AvatarFallback>{icon}</AvatarFallback>
				</Avatar>
				<input
					id="dropzone-file"
					className="hidden"
					accept="image/jpeg, image/png"
					type="file"
					{...getInputProps()}
				/>
				{cropProps && (
					<Dialog
						open={!!state.cropUrl}
						onOpenChange={(open) => {
							if (!open) {
								state.cropUrl = undefined;
							}
						}}
					>
						<DialogContent className="flex h-[420px] w-[420px] flex-col items-center justify-center p-4">
							<DialogHeader>
								<DialogTitle>头像裁剪</DialogTitle>
							</DialogHeader>
							<DialogDescription className="relative h-[400px] w-[400px] items-center justify-center">
								<Cropper
									aspect={1}
									{...cropProps}
									image={state.cropUrl}
									crop={state.crop}
									zoom={state.zoom}
									onCropChange={(crop: { x: number; y: number }) => {
										state.crop = crop;
									}}
									onZoomChange={(zoom: number) => {
										state.zoom = zoom;
									}}
									onCropComplete={(
										_area: unknown,
										croppedAreaPixels: unknown,
									) => {
										state.croppedAreaPixels = croppedAreaPixels;
									}}
								/>
							</DialogDescription>
							<DialogFooter className="w-full justify-end">
								<Button
									type="button"
									variant="secondary"
									onClick={() => {
										state.cropUrl = undefined;
									}}
								>
									取消
								</Button>
								<Button
									className="ml-1"
									type="button"
									variant="default"
									onClick={async () => {
										try {
											const croppedImage = await getCroppedImg(
												state.cropUrl!,
												state.croppedAreaPixels,
											);
											state.cropUrl = undefined;
											const url = await onUpload(
												new File([croppedImage!], "avatar.jpeg"),
											);
											onChange?.(url);
										} catch (e) {
											console.error(e);
										}
									}}
								>
									确定
								</Button>
							</DialogFooter>
						</DialogContent>
					</Dialog>
				)}
				<div
					className="absolute left-0 top-0 hidden h-full w-full items-center justify-center rounded-full group-hover:flex group-hover:bg-black group-hover:bg-opacity-20"
					{...getRootProps()}
				>
					<Button
						variant="secondary"
						size="sm"
						onClick={(e) => e.preventDefault()}
					>
						上传头像
					</Button>
				</div>
			</div>
		);
	},
);

export { AvatarUploader };
