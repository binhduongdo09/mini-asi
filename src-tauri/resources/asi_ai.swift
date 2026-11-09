// Mini-Asi ↔ Apple Intelligence (FoundationModels, macOS 26+, Apple Silicon). Chạy hoàn toàn trên máy.
//
//   --check          → in "READY" hoặc "UNAVAILABLE:<lý do>"
//   (mặc định)       → đọc ASI_SYSTEM (chỉ dẫn) và ASI_PROMPT (lịch sử + câu hỏi) từ biến môi trường,
//                      in MỘT dòng JSON: {"reply": "...", "action": "...", "argument": "..."} hoặc {"error": "..."}
//
// Biên dịch với `-D ASI_STRUCTURED` → dùng "guided generation" (@Generable): mô hình vừa trả lời vừa chọn hành động
// cho app (thêm việc, ghi chú…). Bộ công cụ không hỗ trợ macro → biên dịch không có cờ đó (chỉ trả lời chữ).
// Lỗi khi sinh có cấu trúc → thử lại bằng câu trả lời chữ thường.
import Foundation
import FoundationModels

#if ASI_STRUCTURED
@Generable
struct AsiTurn {
    @Guide(description: "The answer shown to the user, in the user's language. Plain text, may use short '- ' bullet lines.")
    var reply: String
    @Guide(description: "Exactly one of: none, add_task, add_note, start_focus, show_today, show_overdue, check_mac, clean_junk, open_tab. Use none unless the user clearly wants the app to act.")
    var action: String
    @Guide(description: "Argument for the action (task text with its time words, note text, minutes, or tab name). Empty when action is none.")
    var argument: String
}
#endif

func emit(_ dict: [String: String]) {
    if let data = try? JSONSerialization.data(withJSONObject: dict), let s = String(data: data, encoding: .utf8) {
        print(s)
    }
}

@main
struct AsiAI {
    static func main() async {
        let env = ProcessInfo.processInfo.environment
        let model = SystemLanguageModel.default
        var ready = false
        switch model.availability {
        case .available: ready = true
        default: ready = false
        }
        if CommandLine.arguments.contains("--check") {
            print(ready ? "READY" : "UNAVAILABLE:\(model.availability)")
            return
        }
        guard ready else {
            emit(["error": "Apple Intelligence chưa bật hoặc đang tải mô hình (Cài đặt hệ thống › Apple Intelligence & Siri)."])
            return
        }
        let instructions = env["ASI_SYSTEM"] ?? "You are Asi, a helpful assistant."
        let prompt = env["ASI_PROMPT"] ?? ""
        guard !prompt.isEmpty else {
            emit(["error": "Câu hỏi trống"])
            return
        }
        let options = GenerationOptions(temperature: 0.5)
        #if ASI_STRUCTURED
        if env["ASI_SIMPLE"] != "1" {
            do {
                let session = LanguageModelSession(model: model, instructions: instructions)
                let r = try await session.respond(to: prompt, generating: AsiTurn.self, options: options)
                emit(["reply": r.content.reply, "action": r.content.action, "argument": r.content.argument])
                return
            } catch {
                // rơi xuống trả lời chữ thường
            }
        }
        #endif
        do {
            let session = LanguageModelSession(model: model, instructions: instructions)
            let r = try await session.respond(to: prompt, options: options)
            emit(["reply": r.content, "action": "none", "argument": ""])
        } catch {
            emit(["error": error.localizedDescription])
        }
    }
}
