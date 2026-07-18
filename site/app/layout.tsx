import type { Metadata } from "next";
import { Schibsted_Grotesk, IM_Fell_DW_Pica, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const schibsted = Schibsted_Grotesk({
  variable: "--font-schibsted",
  subsets: ["latin"],
  display: "swap",
  weight: ["400", "500", "600", "700", "800", "900"],
});

const imFell = IM_Fell_DW_Pica({
  variable: "--font-im-fell",
  subsets: ["latin"],
  display: "swap",
  weight: ["400"],
  style: ["normal", "italic"],
});

const jetbrains = JetBrains_Mono({
  variable: "--font-jetbrains",
  subsets: ["latin"],
  display: "swap",
  weight: ["400", "500", "600"],
});

export const metadata: Metadata = {
  title: "Cairn — versioned belief-management for AI coding agents",
  description:
    "A local-first coordination and context substrate for AI agents working inside real software repositories. Not a code index. A ledger of what each agent has seen, when it saw it, and whether that belief has expired.",
  metadataBase: new URL("https://cairn-explainer.vercel.app"),
  openGraph: {
    title: "Cairn",
    description:
      "A local-first coordination and context substrate for AI coding agents. Not a code index — a versioned belief-management system.",
    type: "website",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${schibsted.variable} ${imFell.variable} ${jetbrains.variable} antialiased`}
    >
      <body className="bg-paper-warm text-ink-deep min-h-[100dvh] ground-topo">
        {children}
      </body>
    </html>
  );
}
