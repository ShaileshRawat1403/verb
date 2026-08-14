package com.example.verb.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.unit.dp
import com.example.verb.ai.AiAssistantState
import com.example.verb.ai.AiProviderSettings

/** Provider-backed assistant. It deliberately has no terminal command execution callback. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AssistantScreen(
    providerSettings: AiProviderSettings,
    prompt: String,
    state: AiAssistantState,
    isKeyboardVisible: Boolean = false,
    onPromptChange: (String) -> Unit,
    onSubmitPrompt: (String) -> Unit,
    onOpenProviderSettings: () -> Unit,
    modifier: Modifier = Modifier
) {
    val isGenerating = state is AiAssistantState.Generating
    val scrollState = rememberScrollState()

    val focusManager = LocalFocusManager.current
    val keyboardController = LocalSoftwareKeyboardController.current

    // Back with the keyboard open hides it first instead of leaving the tab.
    BackHandler(enabled = isKeyboardVisible) {
        keyboardController?.hide()
        focusManager.clearFocus(force = true)
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(horizontal = 16.dp)
            .verticalScroll(scrollState)
    ) {
        Spacer(modifier = Modifier.height(24.dp))
        Text(
            text = "AI Assistant",
            style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Bold
        )
        Text(
            text = "Ask, plan, and explain. The assistant cannot execute terminal commands.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
        )

        ProviderStatusCard(
            settings = providerSettings,
            onOpenProviderSettings = onOpenProviderSettings
        )

        Spacer(modifier = Modifier.height(16.dp))
        OutlinedTextField(
            value = prompt,
            onValueChange = onPromptChange,
            modifier = Modifier
                .fillMaxWidth()
                .testTag("assistant_prompt_input"),
            label = { Text("Ask your assistant") },
            placeholder = { Text("Explain a command, plan a task, or ask a question") },
            minLines = 4,
            maxLines = 8,
            enabled = providerSettings.isReady && !isGenerating,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
            keyboardActions = KeyboardActions(
                onSend = {
                    if (prompt.isNotBlank() && !isGenerating) onSubmitPrompt(prompt)
                }
            )
        )

        Spacer(modifier = Modifier.height(12.dp))
        Button(
            onClick = { onSubmitPrompt(prompt) },
            modifier = Modifier
                .fillMaxWidth()
                .testTag("assistant_submit_button"),
            enabled = providerSettings.isReady && prompt.isNotBlank() && !isGenerating
        ) {
            if (isGenerating) {
                CircularProgressIndicator(
                    modifier = Modifier.height(20.dp),
                    strokeWidth = 2.dp,
                    color = MaterialTheme.colorScheme.onPrimary
                )
            } else {
                Icon(Icons.AutoMirrored.Filled.Send, contentDescription = null)
                Spacer(modifier = Modifier.width(8.dp))
                Text("Ask provider")
            }
        }

        when (state) {
            AiAssistantState.Idle,
            AiAssistantState.Generating -> Unit

            is AiAssistantState.Answer -> {
                Spacer(modifier = Modifier.height(20.dp))
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(16.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text("Assistant response", fontWeight = FontWeight.Bold)
                        Text(
                            "${state.response.providerId.displayName} · ${state.response.model}",
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 2.dp, bottom = 10.dp)
                        )
                        Text(state.response.text)
                    }
                }
            }

            is AiAssistantState.Failure -> {
                Spacer(modifier = Modifier.height(20.dp))
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(16.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer)
                ) {
                    Text(
                        state.message,
                        color = MaterialTheme.colorScheme.onErrorContainer,
                        modifier = Modifier.padding(16.dp)
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(24.dp))
    }
}

@Composable
private fun ProviderStatusCard(
    settings: AiProviderSettings,
    onOpenProviderSettings: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text("Provider", fontWeight = FontWeight.Bold)
            if (settings.isReady) {
                Text(
                    "${settings.config!!.providerId.displayName} · ${settings.config.model}",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp, bottom = 12.dp)
                )
            } else {
                Text(
                    "No provider is configured yet. Add your own API key in System.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp, bottom = 12.dp)
                )
            }
            OutlinedButton(onClick = onOpenProviderSettings) {
                Icon(Icons.Default.Key, contentDescription = null)
                Spacer(modifier = Modifier.width(8.dp))
                Text(if (settings.isReady) "Manage provider" else "Configure provider")
            }
        }
    }
}
