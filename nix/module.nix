{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.bakasync;

  settings = {
    BAKASYNC_BASE_URL = cfg.baseUrl;
    BAKASYNC_CLASS_ID = cfg.classId;
    BAKASYNC_WEEKS = lib.concatStringsSep "," cfg.weeks;
    BAKASYNC_GROUPS = lib.concatStringsSep "," cfg.groups;
    BAKASYNC_CALENDAR_ID = cfg.calendarId;
    BAKASYNC_SERVICE_ACCOUNT_KEY = "%d/sa.json";
    BAKASYNC_MERGE_GAP_MINUTES = toString cfg.mergeGapMinutes;
    BAKASYNC_INTERVAL = cfg.interval;
  }
  // lib.optionalAttrs (cfg.expectedClassName != null) {
    BAKASYNC_EXPECTED_CLASS_NAME = cfg.expectedClassName;
  }
  // cfg.extraSettings;
in
{
  options.services.bakasync = {
    enable = lib.mkEnableOption "the Bakalari to Google Calendar sync";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The bakasync package to run.";
    };

    baseUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://bakalari.example.cz/bakaweb";
      description = "Root of the Bakalari web application.";
    };

    classId = lib.mkOption {
      type = lib.types.str;
      example = "2T";
      description = "Internal class id used in the public timetable URL, not the class name.";
    };

    expectedClassName = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "3.D";
      description = "Class name expected on the page; a mismatch is logged as a warning.";
    };

    weeks = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        "Actual"
        "Next"
      ];
      description = "Timetable weeks to scrape.";
    };

    groups = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [
        "S1"
        "PX2"
      ];
      description = "Split groups to keep; whole class lessons are always kept.";
    };

    calendarId = lib.mkOption {
      type = lib.types.str;
      example = "abc123@group.calendar.google.com";
      description = "Id of the dedicated Google Calendar to write to.";
    };

    serviceAccountKeyFile = lib.mkOption {
      type = lib.types.path;
      example = "/run/secrets/bakasync-sa.json";
      description = "Google service account JSON key, passed to the service as a credential.";
    };

    interval = lib.mkOption {
      type = lib.types.str;
      default = "10min";
      description = "Delay between runs; an empty value makes the service run once and exit.";
    };

    mergeGapMinutes = lib.mkOption {
      type = lib.types.int;
      default = 25;
      description = "Largest break that still merges two consecutive lessons into one event.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Extra environment file read by systemd, for values kept out of the store.";
    };

    extraSettings = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      example = {
        RUST_LOG = "debug";
      };
      description = "Additional environment variables for the service.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.bakasync = {
      description = "Bakalari timetable to Google Calendar sync";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];
      environment = settings;
      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        LoadCredential = [ "sa.json:${toString cfg.serviceAccountKeyFile}" ];
        EnvironmentFile = lib.optional (cfg.environmentFile != null) cfg.environmentFile;
        Restart = "on-failure";
        RestartSec = 60;
        DynamicUser = true;
        CapabilityBoundingSet = [ "" ];
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateTmp = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHome = true;
        ProtectHostname = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectSystem = "strict";
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [ "@system-service" ];
        UMask = "0077";
      };
    };
  };
}
