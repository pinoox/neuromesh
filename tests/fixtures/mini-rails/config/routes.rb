Rails.application.routes.draw do
  post '/sms', to: 'sms#create'
end
