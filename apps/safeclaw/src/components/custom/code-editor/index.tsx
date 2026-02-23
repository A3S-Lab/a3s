import { Editor, EditorProps, Monaco } from "@monaco-editor/react";
import { useRef } from "react";
import * as MonacoEditor from "monaco-editor";
import ThemeOneDarkPro from "./themes/onedarkpro.json";

export default function CodeEditor(props: EditorProps) {
	const editorRef = useRef<MonacoEditor.editor.IStandaloneCodeEditor>();

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
	}

	return (
		<Editor
			theme="one-dark-pro"
			height="100%"
			beforeMount={handleBeforeMount}
			options={{
				fontSize: 16,
				fontFamily: "'Fira Code', 'Fira Code VF', monospace",
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
			}}
			{...props}
		/>
	);
}
