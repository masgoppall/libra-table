//! What each Redis command does, for safe mode and read-only connections: commands
//! flagged `readonly` by Redis 7's `COMMAND INFO` (plus harmless server commands) are
//! reads; connection state (SELECT, MULTI/EXEC, AUTH…) is session; everything else,
//! including unknown and module commands, is a write (fail safe).

use crate::safe_mode::Kind;

use super::args;

/// Read-only commands, by group.
const READ: &str = "
    GET GETRANGE SUBSTR MGET STRLEN LCS GETBIT BITCOUNT BITPOS BITFIELD_RO PFCOUNT
    EXISTS TYPE TTL PTTL EXPIRETIME PEXPIRETIME KEYS SCAN RANDOMKEY DBSIZE DUMP TOUCH SORT_RO
    HGET HMGET HGETALL HKEYS HVALS HLEN HEXISTS HSTRLEN HSCAN HRANDFIELD
    HTTL HPTTL HEXPIRETIME HPEXPIRETIME
    LRANGE LINDEX LLEN LPOS
    SMEMBERS SISMEMBER SMISMEMBER SCARD SSCAN SRANDMEMBER SINTER SUNION SDIFF SINTERCARD
    ZRANGE ZRANGEBYSCORE ZRANGEBYLEX ZREVRANGE ZREVRANGEBYSCORE ZREVRANGEBYLEX ZSCORE
    ZMSCORE ZRANK ZREVRANK ZCARD ZCOUNT ZLEXCOUNT ZSCAN ZRANDMEMBER ZINTER ZUNION ZDIFF
    ZINTERCARD
    XRANGE XREVRANGE XLEN XREAD XINFO XPENDING
    GEOPOS GEODIST GEOHASH GEORADIUS_RO GEORADIUSBYMEMBER_RO GEOSEARCH
    EVAL_RO EVALSHA_RO FCALL_RO
    PING ECHO INFO TIME LASTSAVE ROLE LOLWUT COMMAND WAIT WAITAOF HELP
    JSON.GET JSON.MGET JSON.TYPE JSON.STRLEN JSON.OBJKEYS JSON.OBJLEN JSON.ARRLEN
    JSON.ARRINDEX JSON.RESP JSON.DEBUG
    FT.SEARCH FT.AGGREGATE FT.INFO FT._LIST FT.EXPLAIN FT.PROFILE FT.TAGVALS FT.SPELLCHECK
    FT.SUGGET FT.SUGLEN FT.DICTDUMP FT.SYNDUMP
    TS.GET TS.MGET TS.RANGE TS.REVRANGE TS.MRANGE TS.MREVRANGE TS.INFO TS.QUERYINDEX
    BF.EXISTS BF.MEXISTS BF.INFO BF.CARD CF.EXISTS CF.MEXISTS CF.COUNT CF.INFO
    CMS.QUERY CMS.INFO TOPK.QUERY TOPK.LIST TOPK.INFO TOPK.COUNT
    TDIGEST.QUANTILE TDIGEST.CDF TDIGEST.MIN TDIGEST.MAX TDIGEST.INFO TDIGEST.RANK
    TDIGEST.REVRANK TDIGEST.BYRANK TDIGEST.BYREVRANK TDIGEST.TRIMMED_MEAN
";

/// Container commands whose subcommand decides: (command, subcommands that only read).
/// XREADGROUP is not here: it moves entries to a consumer, a write.
const READ_SUBCOMMANDS: &[(&str, &str)] = &[
    ("CONFIG", "GET HELP"),
    ("CLIENT", "LIST INFO GETNAME ID TRACKINGINFO GETREDIR HELP"),
    ("OBJECT", "ENCODING FREQ IDLETIME REFCOUNT HELP"),
    ("MEMORY", "USAGE STATS DOCTOR MALLOC-STATS HELP"),
    ("SLOWLOG", "GET LEN HELP"),
    ("LATENCY", "LATEST HISTORY DOCTOR GRAPH HISTOGRAM HELP"),
    (
        "ACL",
        "WHOAMI LIST USERS GETUSER CAT LOG GENPASS DRYRUN HELP",
    ),
    ("MODULE", "LIST HELP"),
    ("FUNCTION", "LIST DUMP STATS HELP"),
    ("SCRIPT", "EXISTS HELP"),
    (
        "PUBSUB",
        "CHANNELS NUMSUB NUMPAT SHARDCHANNELS SHARDNUMSUB HELP",
    ),
    (
        "CLUSTER",
        "INFO NODES SLOTS SHARDS MYID MYSHARDID KEYSLOT COUNTKEYSINSLOT GETKEYSINSLOT LINKS \
         REPLICAS SLAVES HELP",
    ),
];

/// Connection state only (no data changes).
const SESSION: &str = "SELECT MULTI EXEC DISCARD WATCH UNWATCH AUTH HELLO RESET READONLY \
    READWRITE QUIT";

const SESSION_CLIENT: &str = "SETNAME SETINFO NO-EVICT NO-TOUCH";

/// Commands that take over the connection (pushes, replication) or silence it.
const NOT_IN_EDITOR: &str = "SUBSCRIBE PSUBSCRIBE SSUBSCRIBE UNSUBSCRIBE PUNSUBSCRIBE \
    SUNSUBSCRIBE MONITOR SYNC PSYNC REPLCONF";

fn listed(words: &str, name: &str) -> bool {
    words.split_whitespace().any(|w| w == name)
}

fn upper(arg: &[u8]) -> String {
    String::from_utf8_lossy(arg).to_ascii_uppercase()
}

/// Safe-mode kind of a parsed command.
pub fn kind_of(cmd: &[Vec<u8>]) -> Kind {
    let Some(name) = cmd.first().map(|a| upper(a)) else {
        return Kind::Write;
    };
    let sub = cmd.get(1).map(|a| upper(a)).unwrap_or_default();
    if listed(READ, &name) {
        return Kind::Read;
    }
    if listed(SESSION, &name) {
        return Kind::Session;
    }
    if name == "CLIENT" && listed(SESSION_CLIENT, &sub) {
        return Kind::Session;
    }
    let read_sub = READ_SUBCOMMANDS
        .iter()
        .find(|(c, _)| *c == name)
        .is_some_and(|(_, subs)| listed(subs, &sub));
    if read_sub {
        Kind::Read
    } else {
        Kind::Write
    }
}

/// Safe-mode kind of one editor line; text that doesn't parse counts as a write.
pub fn classify(statement: &str) -> Kind {
    match args::parse(statement) {
        Ok(cmd) if !cmd.is_empty() => kind_of(&cmd),
        _ => Kind::Write,
    }
}

/// Why the editor won't send this command, if it won't.
pub fn refused(cmd: &[Vec<u8>]) -> Option<String> {
    let name = upper(cmd.first()?);
    let sub = cmd.get(1).map(|a| upper(a)).unwrap_or_default();
    if listed(NOT_IN_EDITOR, &name) || (name == "CLIENT" && sub == "REPLY") {
        return Some(format!(
            "{name} takes over the connection, so it can't run in the query editor."
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_writes_and_session() {
        for read in [
            "GET k",
            "hgetall h",
            "SCAN 0 MATCH a* COUNT 10",
            "TYPE k",
            "TTL k",
            "INFO keyspace",
            "CONFIG GET maxmemory",
            "client list",
            "OBJECT ENCODING k",
            "XRANGE s - +",
            "JSON.GET doc",
        ] {
            assert_eq!(classify(read), Kind::Read, "{read}");
        }
        for write in [
            "SET k v",
            "DEL k",
            "FLUSHALL",
            "CONFIG SET maxmemory 1",
            "CLIENT KILL ID 4",
            "EVAL \"return 1\" 0",
            "XREADGROUP GROUP g c STREAMS s >",
            "MYMODULE.DO x",
            "SET k \"open",
            "",
        ] {
            assert_eq!(classify(write), Kind::Write, "{write}");
        }
        for session in ["SELECT 2", "multi", "EXEC", "CLIENT SETNAME x", "WATCH k"] {
            assert_eq!(classify(session), Kind::Session, "{session}");
        }
    }

    #[test]
    fn refuses_connection_takeovers() {
        let cmd = args::parse("subscribe news").unwrap();
        assert!(refused(&cmd).is_some());
        assert!(refused(&args::parse("CLIENT REPLY OFF").unwrap()).is_some());
        assert!(refused(&args::parse("GET k").unwrap()).is_none());
    }
}
