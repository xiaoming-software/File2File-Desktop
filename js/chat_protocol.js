/**
 * Shared chat application-layer protocol (App + Desktop).
 *
 * Types: 1 hello (+chatProto/caps), 2 text {msgId,text,ts}, 3/4 file,
 * 20 delivery_ack {msgId}, 21 ping {t}, 22 pong {t},
 * 25 read_receipt {msgIds:[]}.
 * Caps: "ack", "read". (12 is reserved by videocall — do not reuse.)
 */
(function (global) {
  var ChatProtocol = {
    HELLO: 1,
    TEXT: 2,
    FILE_QUERY: 3,
    FILE_REPLY: 4,
    DELIVERY_ACK: 20,
    PING: 21,
    PONG: 22,
    READ_RECEIPT: 25,
    CHAT_PROTO: 1,
    CAP_ACK: "ack",
    CAP_READ: "read",
    HEARTBEAT_MS: 15000,
    HEARTBEAT_MISS_LIMIT: 3,
    localCaps: function () {
      return [ChatProtocol.CAP_ACK, ChatProtocol.CAP_READ];
    },
    capsIncludeAck: function (caps) {
      if (!Array.isArray(caps)) return false;
      return caps.indexOf(ChatProtocol.CAP_ACK) >= 0;
    },
    capsIncludeRead: function (caps) {
      if (!Array.isArray(caps)) return false;
      return caps.indexOf(ChatProtocol.CAP_READ) >= 0;
    },
    helloExtras: function () {
      return { chatProto: ChatProtocol.CHAT_PROTO, caps: ChatProtocol.localCaps() };
    },
    textPayload: function (msgId, text, ts) {
      return {
        msgId: String(msgId || ""),
        text: text == null ? "" : String(text),
        ts: ts || Date.now(),
      };
    },
    ackPayload: function (msgId) {
      return { msgId: String(msgId || "") };
    },
    readPayload: function (msgIds) {
      var ids = Array.isArray(msgIds) ? msgIds : [];
      return {
        msgIds: ids
          .map(function (id) {
            return String(id || "");
          })
          .filter(Boolean),
      };
    },
    pingPayload: function (t) {
      return { t: Number(t) || Date.now() };
    },
    pongPayload: function (t) {
      return { t: Number(t) || Date.now() };
    },
  };
  global.ChatProtocol = ChatProtocol;
})(typeof window !== "undefined" ? window : globalThis);
