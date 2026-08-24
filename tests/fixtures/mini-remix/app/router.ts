import { createBrowserRouter } from "react-router";
import { action } from "./routes/sms";

export const router = createBrowserRouter([{ path: "/sms", action }]);
