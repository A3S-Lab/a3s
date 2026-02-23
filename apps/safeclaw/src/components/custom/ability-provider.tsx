import {
	ForcedSubject,
	MongoAbility,
	RawRuleOf,
	createMongoAbility,
} from "@casl/ability";
import { createContextualCan } from "@casl/react";
import { createContext, useContext, useMemo } from "react";

export const actions = [
	"manage",
	"create",
	"read",
	"update",
	"delete",
] as const;

export type Abilities = [
	(typeof actions)[number],
	string | ForcedSubject<Exclude<string, "all">>,
];
export type AppAbility = MongoAbility<Abilities>;
export const createAbility = (rules: RawRuleOf<AppAbility>[]) =>
	createMongoAbility<AppAbility>(rules);

export const AbilityContext = createContext<AppAbility>(createAbility([]));
export const Can = createContextualCan(AbilityContext.Consumer);

type AbilityProviderProps = {
	children: React.ReactNode;
	rules: RawRuleOf<AppAbility>[];
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
		can: (
			action: (typeof actions)[number],
			subject: string | ForcedSubject<Exclude<string, "all">>,
		) => context.can(action, subject),
		cannot: (
			action: (typeof actions)[number],
			subject: string | ForcedSubject<Exclude<string, "all">>,
		) => context.cannot(action, subject),
	};
};
