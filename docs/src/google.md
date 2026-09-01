# Google

1. Enable the Calendar API in a GCP project.
2. Create a service account. Download its JSON key.
3. Create a calendar for bakasync alone.
4. Share it with the service account e-mail. Permission: **Make changes to events**.

No OAuth, no browser. The service account signs its own token, so the sync can run unattended.

Step 3 is not optional advice. Point it at a calendar you also use by hand and you lose the one thing
that keeps your own events safe: a calendar it fully owns.
