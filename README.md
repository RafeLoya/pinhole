```
██████╗ ██╗███╗   ██╗██╗  ██╗ ██████╗ ██╗     ███████╗
██╔══██╗██║████╗  ██║██║  ██║██╔═══██╗██║     ██╔════╝
██████╔╝██║██╔██╗ ██║███████║██║   ██║██║     █████╗  
██╔═══╝ ██║██║╚██╗██║██╔══██║██║   ██║██║     ██╔══╝  
██║     ██║██║ ╚████║██║  ██║╚██████╔╝███████╗███████╗
╚═╝     ╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚══════╝
```

---

# About

Pinhole is a video chat application that functions completely within a shell.

The video feed from two peers in the same session is forwarded between one another in a custom UTF-8 character representation. With just a network, a shell, and a way to record I-frames, you can send, receive, and render live video!

This repository contains a server and client binary, where a server facilitates the actual connection between two clients and the forwarding of their video data. End users will likely want to use the client executable, provided a server is up and running.

# Requirements

The client binary directly uses the FFmpeg CLI, which can be downloaded from the [official website](https://ffmpeg.org/download.html). The website itself only provides the source code, but if you are not interested in building from source, links are provided to find it as an executable.

# Installation

## Building From Source

After cloning the repository, build with `cargo` with a release flag and use the executable(s) as you see fit:

```shell
cargo build --release

# or, if you are only interested in one executable:
cargo build --release --bin client
cargo build --release --bin server
```

---

[![Review Assignment Due Date](https://classroom.github.com/assets/deadline-readme-button-22041afd0340ce965d47ae6ef1cefeee28c7c493a6346c4f15d667ab976d596c.svg)](https://classroom.github.com/a/6FRwiRqU)
Goal: Apply the knowledge you've learned in new ways.

# Project description
This is an open-ended project. Students can extend their BearTV project or do something new from the ground up. Project ideas must be approved by Dr. Freeman.

You must give a **formal presentation** of your project in place of a final exam. Each group will have ~12 minutes to present their work. Each member of the group must speak. You should have slides. Your presentation must include a demo of your project, although it may invlude a pre-recorded screen capture. In your presentation, you should introduce the problem that you addressed, how you addressed it, technical challenges you faced, what you learned, and next steps (if you were to continue developing it).

You may use AI LLM tools to assist with the development of your project, including code assistant tools like GitHub Copilot. If you do use any AI tools, you must describe your use during your presentation.

Unless you get specific approval otherwise, your project **must** include some component deployed on a cloud hosting service. You can use AWS, GCP, Azure, etc. These services have free tiers, and you might consider looking into tiers specifically for students.

## Milestones
- You must meet with Dr. Freeman within the first week to get your project idea approved
- You must meet with Dr. Freeman within the first 3 weeks to give a status update and discuss roadblocks
- See the course schedule spreadhseet for specific dates

## Project Ideas
- Simulate UDP packet loss and packet corruption in BearTV in a non-deterministic way (i.e., don't just drop every Nth packet). Then, extend the application protocol to be able to detect and handle this packet loss.
- Extend the BearTV protocol to support streaming images (or video!) alongside the CC data, and visually display them on the client. This should be done in such a way that it is safely deliver*able* over *any* implementation of IPv4. The images don't have to be relevant to the caption data--you can get them randomly on the server from some image source.
- Do something hands on with a video streaming protocol such as MoQ, DASH, or HLS.
- Implement QUIC
- Develop a new congestion control algorithm and evaluate it compared to existing algorithms in a realistic setting
- Make significant contributions to a relevant open-source repository (e.g., moq-rs)
- Implement a VPN
- Implement a DNS
- Do something with route optimization
- Implement an HTTP protocol and have a simple website demo

--> These are just examples. I hope that you'll come up with a better idea to suit your own interests!

## Libraries

Depending on the project, there may be helpful libraries you find to help you out. However, there may also be libraries that do all the interesting work for you. Depending on the project, you'll need to determine what should be fair game. For example, if your project is to implement HTTP, then you shouldn't leverage an HTTP library that does it for you.

If you're unsure if a library is okay to use, just ask me.

## Languages

The core of your project should, ideally, be written in Rust. Depending on the project idea, however, I'm open to allowing the use of other languages if there's a good reason for it. For me to approve such a request, the use of a different language should enable greater learning opportunities for your group.

# Submission

## Questions
- What is your project?
  - A two-way video chat application capable of live-streaming video as an ASCII representation within a shell. To ensure the connections between peers, we utilize a cloud server acting as a Selective Forwarding Unit with some NAT traversal capabilities similar to a STUN server.
- What novel work did you do?
  - Custom video coding format, which encodes and decodes video frames into frames of UTF-8 characters
  - Rust implementation of various image processing / computer vision algorithms
    - [Sobel operator](https://en.wikipedia.org/wiki/Sobel_operator)
    - [Linear grayscale conversion](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.601-7-201103-I!!PDF-E.pdf)
    - [Non-maximum suppression (gradient magnitude thresholding)](https://en.wikipedia.org/wiki/Canny_edge_detector)
  - The combination of traditional ASCII art with edge detection to preserve details (*i.e. edge information*) that would be lost in translation.
  - SFU server with NAT traversal capabilities similar to a STUN server
    - A peer-pairing media relay server with dual-protocol channels (TCP for control, UDP for data forwarding) and connection state management
  - Terminal-based video chat system, which could be used in environments with poor / nonexistent graphical interfaces
- What did you learn?
  - Various image processing / computer vision concepts
  - How to utilize FFmpeg's capabilities in various applications
  - Different server architectures and NAT traversal methods commonly used in video conferencing / live streaming applications
  - A deeper understanding of Rust
- What was challenging?
  - Negotiating with operating systems to allow our program to use their webcams
  - Making the ASCII art representation better than just "grayscale"
  - Communication and cooperation between Tokio threads
  - Designing the server architecture
  - Debugging and refining session logic
- What AI tools did you use, and what did you use them for? What were their benefits and drawbacks?
  - ChatGPT and Claude were used extensively for multiple reasons:
    - Assisting with learning new concepts / topics
    - Exploring alternative methods of implementing certain program functionality
    - Debugging subtle errors in the program's logic or code
    - Sparingly, and after careful review and / or modification, generated code snippets were utilized
  - Both of these tools had significant drawbacks:
    - For both, they often struggled to understand the program or give meaningful feedback
      - This may be due to poor prompting on our part
    - For ChatGPT in particular, with the o3 model, it was very persistent about certain responses it would generate, even after being proven wrong
    - Rabbitholes due to misleading suggestions and / or hallucinations
- What would you do differently next time?
  - Work on the server and protocol sooner, they were more difficult to test and debug than the Video-To-ASCII pipeline
  - Have a more structured approach to communication and achieving project milestones

## What to submit
- Push your working code to the main branch of your team's GitHub Repository before the deadline
- Edit the README to answer the above questions
- On Teams, *each* member of the group must individually upload answers to these questions:
	- What did you (as an individual) contribute to this project?
	- What did the other members of your team contribute?
	- Do you have any concerns about your own performance or that of your team members? Any comments will remain confidential, and Dr. Freeman will try to address them in a way that preserves anonymity.
	- What feedback do you have about this course?

## Grading

Grading will be based on...
- The technical merit of the group's project
- The contribution of each individual group member
- Evidence of consistent work, as revealed during milestone meetings
- The quality of the final presentation
