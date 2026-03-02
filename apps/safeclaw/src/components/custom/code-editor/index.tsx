import { Editor, EditorProps, Monaco } from "@monaco-editor/react";
import ThemeOneDarkPro from "./themes/onedarkpro.json";

export default function CodeEditor(props: EditorProps) {
	function handleBeforeMount(monaco: Monaco) {
		monaco.editor.defineTheme("one-dark-pro", {
			base: "vs-dark",
			inherit: true,
			rules: [...ThemeOneDarkPro.rules],
			encodedTokensColors: [...ThemeOneDarkPro.encodedTokensColors],
			colors: {
				...ThemeOneDarkPro.colors,
			},
		});
		props.beforeMount?.(monaco);
	}

	return (
		<Editor
			theme="one-dark-pro"
			height="100%"
			beforeMount={handleBeforeMount}
			options={{
				fontSize: 14,
				fontFamily: "'Maple Mono NF CN', 'Fira Code', monospace",
				fontLigatures: true,
				wordWrap: "on",
				minimap: {
					enabled: false,
				},
				bracketPairColorization: {
					enabled: true,
				},
				cursorBlinking: "expand",
				formatOnPaste: true,
				suggest: {
					showFields: false,
					showFunctions: false,
				},
				codeLens: true,
				contextmenu: false,
				stickyScroll: { enabled: false },
			}}
			{...props}
		/>
	);
}
