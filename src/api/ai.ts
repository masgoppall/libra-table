// The SQL assistant: its API key (kept in the Keychain by Rust) and asking for a query.

import { type AiAnswer, type AiRequest } from "@/types";
import { call } from "./client";

export const aiApi = {
  aiKeyStatus: () => call<boolean>("ai_key_status"),
  aiSetKey: (key: string) => call<void>("ai_set_key", { key }),
  aiClearKey: () => call<void>("ai_clear_key"),
  aiAsk: (request: AiRequest) => call<AiAnswer>("ai_ask", { request }),
};
