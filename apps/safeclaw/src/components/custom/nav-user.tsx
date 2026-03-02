import { User } from "lucide-react";

export function NavUser() {
	return (
		<div className="flex items-center gap-2 rounded-md border px-2 py-1.5 text-xs text-muted-foreground">
			<User className="size-3.5" />
			<span>Local User</span>
		</div>
	);
}
