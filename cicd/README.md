# CICD Setup Instructions

Instructions were adapted from SEII deployment.

1. Go to <https://console.cloud.google.com>
1. Enable the GCP free trial by hitting the "try for free" button on the home page and following the instructions.
   1. Choose "individual" as the account type.
   1. Enter a credit card or debit card. As the page says, "We ask you for your credit card to make sure you are not a robot. If you use a credit or debit card, you won't be charged unless you manually activate your full account."
1. Activating the free trial should have created a project called "My First Project". Projects are simply a way to organize cloud resources in GCP. If you want to use a different project or create a new one, click the dropdown at the top of the screen
1. In the search bar, search for "Compute Engine". This should take you to a page called "Compute Engine API". Click the **enable** button
1. Create a Linux VM in GCP. This will be your production server
   1. Search for "Add VM Instance". This should open the instance creation page
   1. Increase VM Disk size:
      1. 'OS and Storage' --> Size (GB) = 32
   1. Update firewall settings:
      1. Check the options for allowing both HTTP and HTTPS traffic to the VM. This will give your VM an external IP address once it is created
   1. Reserve a static external IP:
      1. Click on 'Networking' -> Scroll down to 'Network interfaces' and edit
      1. Scroll down to 'External IPv4 address', click it, then click reserve static external IP
   1. Click the "Create" button
1. Navigate to the "VM instances" tab and click the "Set up firewall rules" option
   1. Select the `default-allow-http` rule, and select "Edit" at the top of the page
   1. In the "TCP Ports" section, add 8080 and 3000. This will allow you to access those ports from your local machine. In the future, if you need to add additional or different HTTP ports, you would do so in the same way. Also, if you want to add HTTPS ports, do so in the same way under the `default-allow-https` rule
      - In the `default-allow-https` rule, port 443 is in the TCP Ports section. Leave it. This is the default port used for HTTPS traffic.
   1. In the "UDP Ports" section, add 443. This is the typical port that the QUIC protocol uses.
1. Reserve a static external IP address (if not done so already) (<https://cloud.google.com/vpc/docs/reserve-static-external-ip-address>)
   1. Search up "IP addresses" and click the one that's under "VPC Network"
   1. Click "Reserve external static IP address"
   1. Enter a name for this address. This is just an internal name to refer to this reserved IP address, and not an actual domain name.
   1. Click "Attached to" and select the VM instance you had created earlier
1. Setup a GitHub runner. The GitHub runner waits for certain actions to happen in your repository and runs a user-defined set of commands when one occurs. We will be using this runner to rebuild and redeploy our project whenever a commit on the `main` branch occurs. First, we need to link the runner to your repository:
   1. Open an SSH terminal to your VM in GCP
   1. Determine which CPU architecture your VM is by running the `lscpu` command. The `Architecture` field will tell you if it is `x86_64` (x86) or `aarch64` (ARM64) for the next step.
   1. Install a GitHub self-hosted runner on the VM using the instructions found in your GitHub Repository at Actions > Runners > New self-hosted runner. Follow the "Download" and "Configure" instructions for Linux on your VM's architecture (likely x86) **EXCEPT FOR THE `./run.sh` COMMAND.** You can leave any settings as their default values when prompted.
   1. Install Docker on your VM, using the following instructions (under "Install using the apt repository"): https://docs.docker.com/engine/install/debian/#install-using-the-repository
   1. Run the following command, giving your GitHub runner permission to use Docker commands: `sudo usermod -aG docker <my-username>` (`<my-username>` is the username that you used to log in via SSH to the VM - if you don't know, look at the command-line prompt, it should say your username to the left of the `@` symbol).
   1. To check if `<my-username>` now has access to Docker run: `sudo -u <my-username> -H docker info`. This command will fail if that user does not have permission.
   1. Finally, configure your GitHub runner to run as a service in the background and automatically restart if it crashes. Run `sudo ./svc.sh install` and `sudo ./svc.sh start`. After you deploy your project, you can check the status by running: `sudo ./svc.sh status`. More Info: https://docs.github.com/en/actions/hosting-your-own-runners/managing-self-hosted-runners/configuring-the-self-hosted-runner-application-as-a-service
