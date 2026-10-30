/**
 * Drive / NAS application-layer protocol (App + Desktop + mywebdisk).
 * Types 23/24 = drive ping/pong. Hello: driveProto + caps driveResume.
 * putFile/getFile may carry offset for resume.
 */
(function (global) {
  var DriveProtocol = {
    PING: 23,
    PONG: 24,
    DRIVE_PROTO: 2,
    CAP_RESUME: "driveResume",
    CAP_RANGE: "driveRange",
    HEARTBEAT_MS: 15000,
    HEARTBEAT_MISS_LIMIT: 3,
    localCaps: function () {
      return [DriveProtocol.CAP_RESUME, DriveProtocol.CAP_RANGE];
    },
    capsIncludeResume: function (caps) {
      if (!Array.isArray(caps)) return false;
      return caps.indexOf(DriveProtocol.CAP_RESUME) >= 0;
    },
    capsIncludeRange: function (caps) {
      if (!Array.isArray(caps)) return false;
      return caps.indexOf(DriveProtocol.CAP_RANGE) >= 0;
    },
    helloExtras: function () {
      return {
        driveProto: DriveProtocol.DRIVE_PROTO,
        caps: DriveProtocol.localCaps(),
      };
    },
    pingPayload: function (t) {
      return { t: Number(t) || Date.now() };
    },
    pongPayload: function (t) {
      return { t: Number(t) || Date.now() };
    },
  };
  global.DriveProtocol = DriveProtocol;
})(typeof window !== "undefined" ? window : globalThis);
