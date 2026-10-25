// Editor support for Redis commands: the command word of each line is highlighted and
// completed from a list of common commands.

import { type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { EditorState, type Extension } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  MatchDecorator,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";

export const REDIS_COMMANDS = `
  APPEND BITCOUNT BITPOS DECR DECRBY GET GETDEL GETEX GETRANGE GETSET INCR INCRBY
  INCRBYFLOAT LCS MGET MSET MSETNX PSETEX SET SETEX SETNX SETRANGE STRLEN
  COPY DEL DUMP EXISTS EXPIRE EXPIREAT EXPIRETIME KEYS MOVE OBJECT PERSIST PEXPIRE PTTL
  RANDOMKEY RENAME RENAMENX RESTORE SCAN SORT TOUCH TTL TYPE UNLINK
  HDEL HEXISTS HGET HGETALL HINCRBY HKEYS HLEN HMGET HRANDFIELD HSCAN HSET HSETNX HSTRLEN
  HVALS BLMOVE BLPOP BRPOP LINDEX LINSERT LLEN LMOVE LPOP LPOS LPUSH LPUSHX LRANGE LREM
  LSET LTRIM RPOP RPUSH RPUSHX SADD SCARD SDIFF SDIFFSTORE SINTER SINTERCARD SISMEMBER
  SMEMBERS SMISMEMBER SMOVE SPOP SRANDMEMBER SREM SSCAN SUNION SUNIONSTORE
  BZPOPMAX BZPOPMIN ZADD ZCARD ZCOUNT ZINCRBY ZINTER ZMSCORE ZPOPMAX ZPOPMIN ZRANDMEMBER
  ZRANGE ZRANGEBYSCORE ZRANK ZREM ZREVRANGE ZREVRANK ZSCAN ZSCORE ZUNION
  XACK XADD XDEL XGROUP XINFO XLEN XPENDING XRANGE XREAD XREADGROUP XREVRANGE XTRIM
  PFADD PFCOUNT PFMERGE GEOADD GEODIST GEOPOS GEOSEARCH
  CLIENT CONFIG DBSIZE ECHO EVAL EVALSHA EXEC DISCARD FLUSHDB INFO MEMORY MULTI PING
  SELECT SLOWLOG TIME UNWATCH WATCH COMMAND ACL
`
  .split(/\s+/)
  .filter(Boolean);

const commandMark = Decoration.mark({ class: "cm-redis-command" });

const decorator = new MatchDecorator({
  regexp: /^\s*([A-Za-z][\w.|-]*)/g,
  decorate: (add, _from, to, match) => add(to - (match[1]?.length ?? 0), to, commandMark),
});

const highlight = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = decorator.createDeco(view);
    }
    update(u: ViewUpdate) {
      this.decorations = decorator.updateDeco(u, this.decorations);
    }
  },
  { decorations: (v) => v.decorations },
);

/** Completes the first word of a line from the command list. */
export function completeCommand(ctx: CompletionContext): CompletionResult | null {
  const word = ctx.matchBefore(/^\s*[A-Za-z.]*/);
  if (!word) return null;
  const start = word.from + (word.text.length - word.text.trimStart().length);
  if (start === ctx.pos && !ctx.explicit) return null;
  return {
    from: start,
    options: REDIS_COMMANDS.map((label) => ({ label, type: "keyword" })),
    validFor: /^[A-Za-z.]*$/,
  };
}

const theme = EditorView.baseTheme({
  ".cm-redis-command": { color: "var(--accent-text)", fontWeight: "600" },
});

export function redisEditor(): Extension {
  return [highlight, theme, EditorState.languageData.of(() => [{ autocomplete: completeCommand }])];
}
