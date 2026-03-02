import { createMongoAbility } from "@casl/ability";
import { createContextualCan } from "@casl/react";
import { createContext, useContext, useMemo, type ReactNode } from "react";

export const actions = [
	"manage",
	"create",
	"read",
	"update",
	"delete",
] as const;

export type Abilities = [(typeof actions)[number], string];
export type AppAbility = {
	can: (action: string, subject: string) => boolean;
	cannot: (action: string, subject: string) => boolean;
};
export const createAbility = (rules: unknown[]) =>
	createMongoAbility(rules) as AppAbility;

export const AbilityContext = createContext<AppAbility>(createAbility([]));
export const Can = createContextualCan(AbilityContext.Consumer);

type AbilityProviderProps = {
	children: ReactNode;
	rules: unknown[];
};

export function AbilityProvider({ children, rules }: AbilityProviderProps) {
	const value = useMemo(() => createAbility(rules), [rules]);

	return (
		<AbilityContext.Provider value={value}>{children}</AbilityContext.Provider>
	);
}

export const useAbility = () => {
	const context = useContext(AbilityContext);

	if (context === undefined)
		throw new Error("useAbility must be used within a AbilityProvider");

	return {
		can: (action: (typeof actions)[number], subject: string) =>
			context.can(action, subject),
		cannot: (action: (typeof actions)[number], subject: string) =>
			context.cannot(action, subject),
	};
};
