import { Check, Copy } from "lucide-react";
import { useCallback, useState } from "react";

export default function CopyButton({ text }: { text: string }) {
	const [copied, setCopied] = useState(false);

	const handleCopy = useCallback(async () => {
		await navigator.clipboard.writeText(text);
		setCopied(true);
		setTimeout(() => setCopied(false), 2000);
	}, [text]);

	return (
		<button
			type="button"
			className="absolute right-2 top-2 hidden group-hover:flex items-center justify-center size-7 rounded-md border bg-background/80 backdrop-blur text-muted-foreground hover:text-foreground transition-colors"
			onClick={handleCopy}
			aria-label="Copy code"
		>
			{copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
		</button>
	);
}
