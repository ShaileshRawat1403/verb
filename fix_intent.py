import re

ar_path = "app/src/main/java/com/example/verb/actions/ActionRegistry.kt"
with open(ar_path, "r") as f:
    ar = f.read()

ar = ar.replace("""        if (!isActionSupported(intent.id)) {
            return ActionResult(
                intentId = intent.id,
                title = "Capability Not Supported",
                summary = "Verb V0.1 does not support the requested capability '${intent.id}'.",
                isSuccess = false,
                errorMessage = "Action not registered in V0 Action Registry.",
                originalIntent = enforcedIntent
            )
        }""", """        if (!isActionSupported(intent.id)) {
            return ActionResult(
                intentId = intent.id,
                title = "Capability Not Supported",
                summary = "Verb V0.1 does not support the requested capability '${intent.id}'.",
                isSuccess = false,
                errorMessage = "Action not registered in V0 Action Registry.",
                originalIntent = intent
            )
        }""")

with open(ar_path, "w") as f: f.write(ar)
print("Intent fixed.")
