import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "StreamForge Control",
  description: "Create and operate StreamForge Kafka pipelines on Kubernetes",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="antialiased">{children}</body>
    </html>
  );
}
