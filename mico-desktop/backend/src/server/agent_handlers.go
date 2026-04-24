package server

import (
	"net/http"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/agent"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func (a *App) registerAgentRoutes(mux *http.ServeMux) {
	mux.HandleFunc("POST /api/agent/run", a.runAgent)
}

func (a *App) runAgent(w http.ResponseWriter, r *http.Request) {
	var request agent.RunRequest
	if !decodeJSON(w, r, &request) {
		return
	}
	response, err := a.agents.Run(r.Context(), request)
	if err != nil {
		a.logs.Record(domain.LogError, "agent", "run", err.Error(), map[string]string{
			"provider": string(request.Provider),
			"message":  excerptForLog(request.Message),
		})
	} else {
		tool := ""
		reason := ""
		if response.ToolCall != nil {
			tool = string(response.ToolCall.Tool)
			reason = response.ToolCall.Reason
		}
		a.logs.Record(domain.LogInfo, "agent", "run", "Agent action applied.", map[string]string{
			"provider": string(request.Provider),
			"tool":     tool,
			"reason":   reason,
		})
	}
	writeResult(w, response, err)
}
