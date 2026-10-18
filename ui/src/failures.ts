// The text of a failed operation target: the typed failure when the core sent one (written in
// the language in force), the shell's own sentence when it did not.
import { adoptionMessage } from "./adoption";
import { usesMessage } from "./consumers";
import type { Catalog } from "./i18n/es";
import { actionErrorText } from "./reasons";
import { settingsMessage } from "./settings";
import type { OperationTargetResult } from "./types";

export function operationFailureText(target: OperationTargetResult, text: Catalog): string {
  if (target.actionError) return actionErrorText(target.actionError, text);
  if (target.adoptionError) return adoptionMessage(target.adoptionError, text);
  if (target.usesError) return usesMessage(target.usesError, text);
  if (target.settingsError) return settingsMessage(target.settingsError, text);
  return target.error ?? text.operations.failed;
}
