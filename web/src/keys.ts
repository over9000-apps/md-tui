export type Mode = "read" | "search" | "help";
export type ThemeName = "ink" | "paper";

export interface KeyState {
  mode: Mode;
  pendingG: boolean;
  outline: boolean;
  theme: ThemeName;
}

export type Command =
  | { type: "none" }
  | { type: "line"; dir: 1 | -1 }
  | { type: "page"; dir: 1 | -1 }
  | { type: "half"; dir: 1 | -1 }
  | { type: "top" }
  | { type: "bottom" }
  | { type: "searchStart" }
  | { type: "searchCommit" }
  | { type: "searchCancel" }
  | { type: "find"; dir: 1 | -1 }
  | { type: "heading"; dir: 1 | -1 }
  | { type: "toggleOutline" }
  | { type: "cycleTheme" }
  | { type: "openHelp" }
  | { type: "closeHelp" };

export interface KeyInput {
  key: string;
  ctrl?: boolean;
}

export function initialState(): KeyState {
  return { mode: "read", pendingG: false, outline: true, theme: "ink" };
}

export function reduce(state: KeyState, input: KeyInput): { state: KeyState; command: Command } {
  const key = input.key;
  if (state.mode === "help") {
    if (key === "Escape" || key === "?" || key === "q") {
      return { state: { ...state, mode: "read", pendingG: false }, command: { type: "closeHelp" } };
    }
    return { state, command: { type: "none" } };
  }
  if (state.mode === "search") {
    if (key === "Escape") {
      return { state: { ...state, mode: "read", pendingG: false }, command: { type: "searchCancel" } };
    }
    if (key === "Enter") {
      return { state: { ...state, mode: "read", pendingG: false }, command: { type: "searchCommit" } };
    }
    return { state, command: { type: "none" } };
  }

  if (state.pendingG && key === "g" && !input.ctrl) {
    return { state: { ...state, pendingG: false }, command: { type: "top" } };
  }

  const base: KeyState = { ...state, pendingG: false };
  if (input.ctrl && (key === "d" || key === "D")) {
    return { state: base, command: { type: "half", dir: 1 } };
  }
  if (input.ctrl && (key === "u" || key === "U")) {
    return { state: base, command: { type: "half", dir: -1 } };
  }
  if (input.ctrl && (key === "f" || key === "F")) {
    return { state: base, command: { type: "page", dir: 1 } };
  }

  switch (key) {
    case "j":
    case "ArrowDown":
      return { state: base, command: { type: "line", dir: 1 } };
    case "k":
    case "ArrowUp":
      return { state: base, command: { type: "line", dir: -1 } };
    case "g":
      return { state: { ...base, pendingG: true }, command: { type: "none" } };
    case "G":
    case "End":
      return { state: base, command: { type: "bottom" } };
    case "Home":
      return { state: base, command: { type: "top" } };
    case "/":
      return { state: { ...base, mode: "search" }, command: { type: "searchStart" } };
    case "n":
      return { state: base, command: { type: "find", dir: 1 } };
    case "N":
      return { state: base, command: { type: "find", dir: -1 } };
    case "d":
      return { state: base, command: { type: "half", dir: 1 } };
    case "u":
      return { state: base, command: { type: "half", dir: -1 } };
    case " ":
    case "PageDown":
      return { state: base, command: { type: "page", dir: 1 } };
    case "b":
    case "PageUp":
      return { state: base, command: { type: "page", dir: -1 } };
    case "[":
      return { state: base, command: { type: "heading", dir: -1 } };
    case "]":
      return { state: base, command: { type: "heading", dir: 1 } };
    case "o":
      return {
        state: { ...base, outline: !state.outline },
        command: { type: "toggleOutline" },
      };
    case "t": {
      const theme: ThemeName = state.theme === "ink" ? "paper" : "ink";
      return { state: { ...base, theme }, command: { type: "cycleTheme" } };
    }
    case "?":
      return { state: { ...base, mode: "help" }, command: { type: "openHelp" } };
    default:
      return { state: base, command: { type: "none" } };
  }
}
