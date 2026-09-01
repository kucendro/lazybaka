# NixOS

```nix
{
  inputs.bakasync.url = "github:kucendro/lazybaka";

  outputs = { nixpkgs, bakasync, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        bakasync.nixosModules.bakasync
        ({ config, ... }: {
          services.bakasync = {
            enable = true;
            baseUrl = "https://bakalari.example.cz/bakaweb";
            classId = "1A";
            expectedClassName = "1.A";
            groups = [ "S1" "PX2" ];
            calendarId = "abc123@group.calendar.google.com";
            serviceAccountKeyFile = config.age.secrets.bakasync-sa.path;
            interval = "10min";
          };
        })
      ];
    };
  };
}
```

Hardened `DynamicUser` service. The key goes in through `LoadCredential`, so it never reaches the Nix
store. Use `environmentFile` for other secrets and `extraSettings` for the rest of the
[table](variables.md).

```console
$ systemctl status bakasync
$ journalctl -u bakasync -f
```
