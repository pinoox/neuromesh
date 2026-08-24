import SwiftUI

struct SmsInbox: View {
    func store(body: String) {
        SmsStore.save(body)
    }

    var body: some View {
        Text("sms")
    }
}
